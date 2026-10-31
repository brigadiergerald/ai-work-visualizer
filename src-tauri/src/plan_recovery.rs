//! Exact first-plan recovery, then a separately claimed original writing stage.
//! Registration never invokes the AI worker. Existing two-turn/unknown guards remain.
use crate::{engine::{Engine,artifact,verify_artifact},model::*,recovery::{self,Contract,sha}};
use serde::Deserialize;
use serde_json::{json,Value};
use std::{fs,path::Path,sync::Arc};

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Grant {
 pub version:u32,pub job_id:String,pub thread_id:String,pub original_state_path:String,pub original_state_sha256:String,
 pub authority_path:String,pub authority_sha256:String,pub criteria_path:String,pub criteria_sha256:String,
 pub proof_path:String,pub proof_sha256:String,pub candidate_sha256:String,pub allow_writing_turn:bool,pub max_turn_starts:u32,
}
fn checked(path:&str,hash:&str)->Result<Vec<u8>,String>{
 if !Path::new(path).is_absolute()||hash.len()!=64{return Err("復旧sourceの絶対path/hashがありません".into())}
 let b=fs::read(path).map_err(|e|e.to_string())?;if sha(&b)!=hash{return Err("復旧sourceのhashが変わりました".into())}Ok(b)
}
fn load(path:&str)->Result<(Grant,Snapshot,Contract,String),String>{
 let bytes=fs::read(path).map_err(|e|e.to_string())?;let g:Grant=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;
 if g.version!=1||g.max_turn_starts!=2||!g.allow_writing_turn{return Err("承認済み1 writing turnの契約ではありません".into())}
 checked(&g.authority_path,&g.authority_sha256)?;checked(&g.criteria_path,&g.criteria_sha256)?;
 let s:Snapshot=serde_json::from_slice(&checked(&g.original_state_path,&g.original_state_sha256)?).map_err(|e|e.to_string())?;
 let c:Contract=serde_json::from_slice(&checked(&g.proof_path,&g.proof_sha256)?).map_err(|e|e.to_string())?;
 let j=s.job.as_ref().ok_or("元Jobなし")?;
 if g.job_id!=j.job_id||Some(g.thread_id.as_str())!=j.thread_id.as_deref()||c.job_id!=j.job_id||c.thread_id!=g.thread_id||c.turns.len()!=1||c.turns[0].node_id!="plan"{return Err("元Job/thread/plan契約不一致".into())}
 if j.status!="failed"||j.active_node_id.as_deref()!=Some("plan")||j.nodes["plan"].status!="failed"||j.nodes["plan"].attempt!=1||!j.artifacts.is_empty(){return Err("元の要件登録失敗ではありません".into())}
 if sha(&fs::read(std::env::current_exe().map_err(|e|e.to_string())?).map_err(|e|e.to_string())?)!=g.candidate_sha256{return Err("承認candidateが一致しません".into())}
 Ok((g,s,c,sha(&bytes)))
}
fn identity(j:&Job,original:&Job)->Result<(),String>{
 if j.job_id!=original.job_id||j.thread_id!=original.thread_id||j.instruction!=original.instruction||j.workspace!=original.workspace||j.mode!="real"||j.workflow_id!="codex-document-v1"||j.collaboration.is_some()||j.recovery.is_some()||serde_json::to_value(&j.workflow).unwrap()!=serde_json::to_value(&original.workflow).unwrap(){return Err("元の文書Job/原文/工程が変わりました".into())}
 let p=j.practical.as_ref().ok_or("契約なし")?;
 if p.turn_starts!=1||p.max_turn_starts!=2||p.original_instruction!=original.instruction||j.nodes["plan"].attempt!=1||["writing","qa","delivery"].iter().any(|n|j.nodes[*n].status!="queued"||j.nodes[*n].attempt!=0){return Err("元の1 turn/未実行writingではありません".into())}Ok(())
}
pub(crate) fn verify(s:&Snapshot,original:&Snapshot,c:&Contract,capture:&Value)->Result<String,String>{
 let j=s.job.as_ref().ok_or("Jobなし")?;let old=original.job.as_ref().ok_or("元Jobなし")?;identity(j,old)?;
 if c.job_id!=j.job_id||Some(c.thread_id.as_str())!=j.thread_id.as_deref()||c.turns.len()!=1||c.turns[0].node_id!="plan"{return Err("元plan契約不一致".into())}
 if original.events.iter().any(|e|!s.events.iter().any(|x|serde_json::to_value(x).unwrap()==serde_json::to_value(e).unwrap())){return Err("元の失敗履歴が欠落しました".into())}
 if capture["thread"]["thread"]["id"]!=c.thread_id||Path::new(capture["thread"]["thread"]["cwd"].as_str().ok_or("元cwdなし")?).canonicalize().map_err(|e|e.to_string())?!=Path::new(&j.workspace).canonicalize().map_err(|e|e.to_string())?{return Err("正式thread/cwd不一致".into())}
 let rows=capture["turns"]["data"].as_array().ok_or("正式turnなし")?;let b=&c.turns[0];
 if !capture["turns"]["nextCursor"].is_null()||rows.len()!=1||rows[0]["id"]!=b.turn_id||rows[0]["status"]!="completed"{return Err("元の正式1 turnではありません".into())}
 let list=&capture["items"][&b.turn_id];let items=list["data"].as_array().ok_or("正式itemなし")?;
 if !list["nextCursor"].is_null()||items.iter().any(|x|x["turnId"]!=b.turn_id){return Err("正式itemが不完全または別turnです".into())}
 let users=items.iter().filter(|x|x["item"]["type"]=="userMessage").collect::<Vec<_>>();let finals=items.iter().filter(|x|x["item"]["type"]=="agentMessage"&&x["item"]["phase"]=="final_answer").collect::<Vec<_>>();
 if users.len()!=1||finals.len()!=1{return Err("正式user/finalが一意ではありません".into())}
 let input=users[0]["item"]["content"].as_array().ok_or("正式requestなし")?;
 if input.len()!=1||input[0]["type"]!="text"{return Err("正式request形式不一致".into())}
 let request=input[0]["text"].as_str().ok_or("正式request本文なし")?;let raw=finals[0]["item"]["text"].as_str().ok_or("正式final本文なし")?;
 if request!=recovery::prompt(original,old,"plan")?||sha(request.as_bytes())!=b.request_sha256||raw.len() as u64!=b.response_bytes||sha(raw.as_bytes())!=b.response_sha256{return Err("元の全文request/final/bytes/hash不一致".into())}
 crate::practical::parse_plan(raw,&j.instruction)?;Ok(raw.into())
}
pub(crate) fn register(e:&Arc<Engine>,original:&Snapshot,c:&Contract,capture:&Value,grant_hash:&str)->Result<(),String>{
 let before=e.snapshot();let raw=verify(&before,original,c,capture)?;let j=before.job.as_ref().unwrap();let old=original.job.as_ref().unwrap();
 let folder=Path::new(&j.workspace).join(".recovery");let bytes=serde_json::to_vec_pretty(capture).map_err(|e|e.to_string())?;let saved=folder.join(format!("plan-official-{}.json",sha(&bytes)));recovery::install_exact(&saved,&bytes)?;
 let audit=folder.join(format!("plan-registration-{grant_hash}.json"));
 let already=before.events.iter().any(|x|x.job_id==j.job_id&&x.event_type=="plan_recovery_verified"&&x.artifact_paths.contains(&audit.to_string_lossy().into_owned()));
 if !already&&serde_json::to_value(j).unwrap()!=serde_json::to_value(old).unwrap(){return Err("元Jobが変わりました。登録前に停止します".into())}
 if already {let a:Value=serde_json::from_slice(&fs::read(&audit).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;if a["grantSha256"]!=grant_hash||a["captureSha256"]!=sha(&bytes)||a["contract"]!=serde_json::to_value(c).unwrap()||a["originalJob"]!=serde_json::to_value(old).unwrap(){return Err("元の復旧監査が変わりました".into())}}
 if !already {let proof=json!({"jobId":j.job_id,"threadId":j.thread_id,"contract":c,"grantSha256":grant_hash,"capturePath":saved,"captureSha256":sha(&bytes),"originalJob":old,"originalFailedRun":old.nodes["plan"],"originalFailedAt":old.ended_at,"registrationInference":0});recovery::install_exact(&audit,&serde_json::to_vec_pretty(&proof).unwrap())?;}
 let plan=crate::practical::parse_plan(&raw,&j.instruction)?;let path=Path::new(&j.workspace).join("plan.json");
 if j.status=="paused"&&j.nodes["plan"].status=="completed"&&already {let a=j.artifacts.first().ok_or("登録済み原planなし")?;verify_artifact(a,Path::new(&j.workspace))?;if j.artifacts.len()!=1||a.node_id!="plan"||a.sha256!=c.turns[0].response_sha256||serde_json::to_value(&j.practical.as_ref().unwrap().plan).unwrap()!=serde_json::to_value(Some(&plan)).unwrap(){return Err("登録済みplanが変わりました".into())}return Ok(())}
 if already {let mut expected=old.clone();expected.status=j.status.clone();expected.nodes.get_mut("plan").unwrap().status=j.nodes["plan"].status.clone();if serde_json::to_value(j).unwrap()!=serde_json::to_value(expected).unwrap(){return Err("復旧待ちの元Jobに別の変更があります".into())}}
 let expected=sha(&serde_json::to_vec(j).unwrap());
 if j.status=="failed" {e.update(|s|{let j=s.job.as_mut().unwrap();if sha(&serde_json::to_vec(j).unwrap())!=expected{return Err("登録前に元Jobが変わりました".into())}j.status="queued".into();j.nodes.get_mut("plan").unwrap().status="queued".into();s.event(Some("plan"),"plan_recovery_verified","queued",format!("元の正式plan {} bytes / SHA-256 {}を再検証。再生成0。失敗記録を保全。",raw.len(),sha(raw.as_bytes())),None,vec![audit.to_string_lossy().into(),saved.to_string_lossy().into()]);Ok(())})?;}
 e.update(|s|{let j=s.job.as_mut().unwrap();if !["queued","running","disconnected"].contains(&j.status.as_str())||!["queued","running","disconnected"].contains(&j.nodes["plan"].status.as_str()){return Err("plan登録工程以外は再開しません".into())}j.status="running".into();j.nodes.get_mut("plan").unwrap().status="running".into();Ok(())})?;
 recovery::install_exact(&path,raw.as_bytes())?;let a=artifact(&path,"plan","text")?;verify_artifact(&a,Path::new(&j.workspace))?;
 e.update(|s|{let j=s.job.as_mut().unwrap();identity(j,old)?;if !j.artifacts.is_empty(){return Err("plan登録が重複しています".into())}j.artifacts.push(a.clone());j.practical.as_mut().unwrap().plan=Some(plan);let run=j.nodes.get_mut("plan").unwrap();run.status="completed".into();run.output=raw;run.error=None;run.ended_at=Some(now());j.status="paused".into();j.active_node_id=None;j.ended_at=None;j.turn_id=None;s.event(Some("plan"),"node_completed","paused","元の要件応答を原bytesのまま登録。文書制作1 turnのみ再開可能。",None,vec![a.path]);s.event(Some("plan"),"plan_recovery_registered","paused",format!("同Job・同thread・plan attempt1を保持。登録時の追加推論0。grant {grant_hash}"),None,vec![audit.to_string_lossy().into()]);Ok(())})
}
pub fn cli(args:&[String])->Result<(),String>{
 if args.len()!=6{return Err("Usage: --recover-plan-formal EXISTING_DATA PROOF GRANT NEW_REPORT".into())}
 let data=Path::new(&args[2]);let report=Path::new(&args[5]);if !data.is_absolute()||!data.join("guild.sqlite3").is_file()||!data.join(".ai-guild-qa").is_file()||!report.is_absolute()||report.exists(){return Err("既存の隔離DBと新規reportが必要です".into())}
 let _lease=fs::OpenOptions::new().create(true).read(true).write(true).open(data.join(".plan-recovery-lease")).map_err(|e|e.to_string())?;_lease.try_lock().map_err(|_|"同Jobの復旧が使用中です")?;
 let(g,original,c,h)=load(&args[4])?;if args[3]!=g.proof_path{return Err("元proofのpath不一致".into())}
 let e=Engine::open(data,None)?;let result=(||{identity(e.snapshot().job.as_ref().ok_or("Jobなし")?,original.job.as_ref().unwrap())?;e.connect()?;let rpc=e.get_rpc()?;let thread=rpc.call("thread/read",json!({"threadId":c.thread_id,"includeTurns":false}))?;let turns=rpc.call("thread/turns/list",json!({"threadId":c.thread_id,"limit":100,"sortDirection":"asc","itemsView":"summary"}))?;let b=&c.turns[0];let item=rpc.call("thread/items/list",json!({"threadId":c.thread_id,"turnId":b.turn_id,"limit":100,"sortDirection":"asc"}))?;let capture=json!({"thread":thread,"turns":turns,"items":{b.turn_id.clone():item}});let(_,_,_,fresh)=load(&args[4])?;if fresh!=h{return Err("待機中に復旧契約が変わりました".into())}register(&e,&original,&c,&capture,&h)})();
 let evidence=json!({"at":now(),"entry":"STANDARD_EXE_SAME_JOB_PLAN_REGISTRATION","result":if result.is_ok(){"PLAN_REGISTERED_NO_INFERENCE"}else{"FAILED"},"error":result.as_ref().err(),"newJobs":0,"newTurns":0,"snapshot":e.snapshot()});recovery::install_exact(report,&serde_json::to_vec_pretty(&evidence).unwrap())?;if let Ok(rpc)=e.get_rpc(){rpc.shutdown()}result
}
pub fn continue_writing(e:&Arc<Engine>,grant_path:&str)->Result<(),String>{
 let(_g,original,c,h)=load(grant_path)?;let s=e.snapshot();let j=s.job.as_ref().ok_or("元Jobなし")?;identity(j,original.job.as_ref().unwrap())?;
 if j.status!="paused"||j.nodes["plan"].status!="completed"||j.artifacts.len()!=1||j.artifacts[0].node_id!="plan"||j.artifacts[0].sha256!=c.turns[0].response_sha256||!s.events.iter().any(|x|x.job_id==j.job_id&&x.event_type=="plan_recovery_registered"&&x.message.contains(&h)){return Err("検証済み原planの続行状態ではありません".into())}
 verify_artifact(&j.artifacts[0],Path::new(&j.workspace))?;
 let claim=Path::new(&j.workspace).join(".recovery").join(format!("writing-{h}.claim"));use std::io::Write;let mut file=fs::OpenOptions::new().write(true).create_new(true).open(claim).map_err(|_|"予定writingの一回限り予約は使用済みです。再送しません")?;file.write_all(b"UNKNOWN_BEFORE_CONNECT; ONE ORIGINAL WRITING TURN ONLY\n").and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;
 e.connect()?;let(_,_,_,fresh)=load(grant_path)?;if fresh!=h{return Err("接続待ち中に続行契約が変わりました".into())}identity(e.snapshot().job.as_ref().unwrap(),original.job.as_ref().unwrap())?;
 e.update(|s|{s.event(Some("writing"),"planned_writing_continuation","paused","承認済みの文書制作1 turnだけを元threadで開始します。原plan再生成0・新Job0。",None,vec![]);Ok(())})?;e.control("resume".into())
}

#[cfg(test)]#[path="plan_recovery_tests.rs"]mod tests;
