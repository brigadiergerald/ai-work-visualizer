//! Same-Job formal-response recovery. No start/resume/steer or model dispatcher.
use crate::{engine::{Engine,verify_artifact},model::*};
use serde::{Serialize,Deserialize};use serde_json::{json,Value};use sha2::{Digest,Sha256};
use std::{fs,path::Path,sync::{Arc,atomic::Ordering}};
pub fn sha(bytes:&[u8])->String{crate::hashing::hex(&Sha256::digest(bytes))}
#[derive(Clone,Serialize,Deserialize,PartialEq,Debug)]#[serde(rename_all="camelCase")]
pub struct Binding{pub node_id:String,pub turn_id:String,pub response_sha256:String,pub response_bytes:u64,pub request_sha256:String}
#[derive(Clone,Serialize,Deserialize,PartialEq,Debug)]#[serde(rename_all="camelCase")]
pub struct Contract{pub job_id:String,pub thread_id:String,pub turns:Vec<Binding>}
#[derive(Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase")]
pub struct Recovery{pub contract:Contract,pub proof_sha256:String,pub capture_path:String,pub capture_sha256:String,pub stage:String,pub started_at:String,pub original_failed_run:Run,pub original_failed_at:Option<String>}
/// Production CLI for the same Engine entry when native input is unavailable.
/// Explicit existing marked data/proof/output paths only; cannot create a Job.
pub fn cli(args:&[String])->Result<(),String>{
 if args.len()!=5{return Err("Usage: --recover-formal EXISTING_DATA PROOF_JSON NEW_REPORT_JSON".into())}
 let data=Path::new(&args[2]);let report=Path::new(&args[4]);
 if !data.is_absolute()||!report.is_absolute()||!data.join("guild.sqlite3").is_file()||!data.join(".ai-guild-qa").is_file()||report.exists(){return Err("既存の専用保存先と新規レポートの絶対パスが必要です".into())}
 let e=Engine::open(data,None)?;
 let result=(||{let proof=fs::read(&args[3]).map_err(|e|e.to_string())?;let c:Contract=serde_json::from_slice(&proof).map_err(|e|e.to_string())?;check_contract(e.snapshot().job.as_ref().ok_or("元Jobなし")?,&c)?;if e.snapshot().job.as_ref().is_none_or(|j|j.status!="completed"){e.connect()?;}e.recover_formal(args[3].clone())})();
 let s=e.snapshot();let evidence=json!({"at":now(),"entry":"STANDARD_CANDIDATE_SAME_ENGINE_RECOVERY_CLI","result":if result.is_ok(){"HOST_RECOVERY_COMPLETED_DISPLAY_SEPARATE"}else{"FAILED"},"error":result.as_ref().err(),"newJobs":0,"newTurns":0,"snapshot":s});
 install_exact(report,&serde_json::to_vec_pretty(&evidence).map_err(|e|e.to_string())?)?;if let Ok(rpc)=e.get_rpc(){rpc.shutdown();}result
}
pub fn install_exact(path:&Path,bytes:&[u8])->Result<(),String>{
 if path.exists(){if fs::read(path).map_err(|e|e.to_string())?==bytes{return Ok(())}return Err(format!("既存ファイルと原bytesが異なります（上書きしません）: {}",path.display()))}
 fs::create_dir_all(path.parent().ok_or("保存先がありません")?).map_err(|e|e.to_string())?;
 let temp=path.with_extension(format!("{}.tmp",id()));
 use std::io::Write;let mut f=fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e|e.to_string())?;
 f.write_all(bytes).and_then(|_|f.sync_all()).map_err(|e|e.to_string())?;drop(f);
 fs::rename(&temp,path).map_err(|e|e.to_string())?;Ok(())
}
fn check_contract(j:&Job,c:&Contract)->Result<(),String>{
 if c.job_id!=j.job_id||Some(c.thread_id.as_str())!=j.thread_id.as_deref()||j.mode!="real"||!["practical-html-v1","codex-document-v1",crate::general::ID].contains(&j.workflow_id.as_str()){return Err("元Job/thread/文書工程が一致しません".into())}
 if c.turns.len()!=2||c.turns[0].node_id!="plan"||c.turns[1].node_id!="writing"||c.turns[0].turn_id==c.turns[1].turn_id||c.turns.iter().any(|b|b.response_bytes==0||b.response_sha256.len()!=64||b.request_sha256.len()!=64){return Err("元の2工程/turn/bytes/hashを特定できません".into())}
 let p=j.practical.as_ref().ok_or("実務契約がありません")?;
 if p.turn_starts!=2||p.max_turn_starts!=2||p.original_instruction!=j.instruction||j.nodes["plan"].status!="completed"||j.nodes["writing"].attempt!=1{return Err("元の実行回数・原文・計画工程が一致しません".into())}
 let pending=j.workflow.nodes.iter().filter(|n|j.nodes[&n.node_id].status!="completed"&&n.node_id!="writing");
 if pending.into_iter().any(|n|!["quality","save"].contains(&n.executor.as_str())){return Err("未実行の推論/手動工程が残っています。復旧はホスト工程だけです".into())}Ok(())
}
pub(crate) fn prompt(s:&Snapshot,j:&Job,node:&str)->Result<String,String>{
 // Reconstruct the original prompt from original instruction/workflow and verified
 // predecessor files, excluding later recovered output. Do not trust proof booleans.
 let mut original=j.clone();original.artifacts.retain(|a|a.kind=="input"||(node=="writing"&&a.node_id=="plan"));
 let n=j.workflow.nodes.iter().find(|n|n.node_id==node).ok_or("元工程なし")?;
 let sources=original.artifacts.iter().map(|a|format!("{} (SHA-256 {})",a.path,a.sha256)).collect::<Vec<_>>().join("\n");
 let additions=s.instructions.iter().filter(|i|i.job_id==j.job_id&&i.kind=="additional").map(|i|i.text.clone()).collect::<Vec<_>>().join("\n");
 let mut prompt_inputs=crate::practical::required_inputs(&original,n)?;for a in original.artifacts.iter().filter(|a|a.kind=="input"){if !prompt_inputs.iter().any(|v|v.path==a.path){verify_artifact(a,Path::new(&j.workspace))?;prompt_inputs.push(a.clone())}}
 let content=prompt_inputs.iter().filter(|a|a.kind!="image").map(|a|fs::read_to_string(&a.path).map(|text|format!("\n--- {} ---\n{}",a.name,text)).map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?.join("\n");
 Ok(format!("ユーザーの指示（原文）：\n{}\n\n追加指示：\n{}\n\n現在の工程：{} / {}\n作業：{}\n\n検証済みの前工程成果物：\n{}\n\n回収した前工程の全文：\n{}\n\nこの工程の完成した回答だけを出力してください。ホストがローカルへ保存します。",j.instruction,additions,n.role,n.label,n.work_instruction,sources,content))
}
pub(crate) fn verify_capture(s:&Snapshot,c:&Contract,capture:&Value)->Result<String,String>{
 let j=s.job.as_ref().ok_or("Jobなし")?;check_contract(j,c)?;
 let thread=&capture["thread"]["thread"];
 if thread["id"]!=c.thread_id||Path::new(thread["cwd"].as_str().ok_or("元cwdなし")?).canonicalize().map_err(|e|e.to_string())?!=Path::new(&j.workspace).canonicalize().map_err(|e|e.to_string())?{return Err("正式threadの所属/保存先不一致".into())}
 let turns=&capture["turns"];
 if !turns["nextCursor"].is_null(){return Err("元turnの一覧が不完全です".into())}
 let rows=turns["data"].as_array().ok_or("正式turn一覧なし")?;
 if rows.len()!=c.turns.len(){return Err("正式turn件数が元Jobと異なります".into())}
 let mut output=String::new();
 for(b,t)in c.turns.iter().zip(rows){
  if t["id"]!=b.turn_id||t["status"]!="completed"{return Err("元turn ID/正式completed不一致".into())}
  let entries=&capture["items"][&b.turn_id];if !entries["nextCursor"].is_null(){return Err("正式itemの一覧が不完全です".into())}
  let rows=entries["data"].as_array().ok_or("正式item一覧なし")?;
  if rows.iter().any(|x|x["turnId"]!=b.turn_id){return Err("別turnのitemです".into())}
  let finals=rows.iter().filter(|x|x["item"]["type"]=="agentMessage"&&x["item"]["phase"]=="final_answer").collect::<Vec<_>>();
  let users=rows.iter().filter(|x|x["item"]["type"]=="userMessage").collect::<Vec<_>>();
  if finals.len()!=1||users.len()!=1{return Err("正式final/user本文が一意ではありません。streamから復元しません".into())}
  let raw=finals[0]["item"]["text"].as_str().ok_or("正式final本文なし")?;
  let request=users[0]["item"]["content"].as_array().ok_or("正式requestなし")?;
  if request.len()!=1||request[0]["type"]!="text"{return Err("元request形式不一致".into())}
  let actual=request[0]["text"].as_str().ok_or("元request本文なし")?;
  if actual!=prompt(s,j,&b.node_id)?||sha(actual.as_bytes())!=b.request_sha256||raw.len() as u64!=b.response_bytes||sha(raw.as_bytes())!=b.response_sha256{return Err("正式request/finalの所属・全文・bytes/SHA-256が一致しません".into())}
  if b.node_id=="plan"{
   let a=j.artifacts.iter().find(|a|a.node_id=="plan"&&a.name=="plan.json").ok_or("元plan未登録")?;verify_artifact(a,Path::new(&j.workspace))?;
   if a.sha256!=b.response_sha256||a.bytes!=b.response_bytes{return Err("元計画fileと正式応答が異なります".into())}
   let plan=if crate::general::applies(j){crate::general::parse_plan(raw,&j.instruction)?}else{crate::practical::parse_plan(raw,&j.instruction)?};
   if serde_json::to_value(&plan).map_err(|e|e.to_string())?!=serde_json::to_value(&j.practical.as_ref().unwrap().plan).map_err(|e|e.to_string())?{return Err("正式計画とJobの要件が異なります".into())}
  }else{output=raw.into();}
 }
 let plan=j.practical.as_ref().and_then(|p|p.plan.as_ref()).ok_or("要件なし")?;if crate::general::applies(j){crate::general::inspect_output(j,&output)?;}else if j.workflow_id=="codex-document-v1"{crate::codex_only::inspect_versioned(&output,plan,&j.instruction,j.practical.as_ref().unwrap().version)?;}else{crate::practical::inspect_html(&output,plan,&j.instruction)?;}Ok(output)
}
impl Engine{
 pub fn recover_formal(self:&Arc<Self>,proof_path:String)->Result<(),String>{
  if self.worker.swap(true,Ordering::SeqCst){return Err("前の処理が実行中です".into())}
  let result=self.recover_formal_inner(&proof_path);
  if let Err(e)=&result{let _=self.update(|s|{if let Some(j)=s.job.as_mut(){if j.recovery.is_some()&&j.status=="running"{j.status="failed".into();if let Some(run)=j.active_node_id.as_ref().and_then(|id|j.nodes.get_mut(id)){if run.status=="running"{run.status="failed".into();run.error=Some(e.clone());}}}}s.event(Some("writing"),"formal_recovery_error","failed",e.clone(),None,vec![]);Ok(())});}
  self.worker.store(false,Ordering::SeqCst);result
 }
 fn recover_formal_inner(&self,proof_path:&str)->Result<(),String>{
  self.require_secret_mask_for_recovery()?;
  let _gate=self.response_gate.lock().map_err(|_|"復旧ロック失敗")?;
  if !Path::new(proof_path).is_absolute(){return Err("元の独立証拠ファイルの絶対パスが必要です".into())}
  let proof=fs::read(proof_path).map_err(|e|e.to_string())?;let c:Contract=serde_json::from_slice(&proof).map_err(|e|e.to_string())?;
  let s=self.snapshot();let j=s.job.as_ref().ok_or("元Jobなし")?;check_contract(j,&c)?;
  if let Some(r)=&j.recovery{if r.contract!=c||r.proof_sha256!=sha(&proof){return Err("既存Recoveryと元証拠が異なります".into())}
   let bytes=fs::read(&r.capture_path).map_err(|e|e.to_string())?;if sha(&bytes)!=r.capture_sha256{return Err("保存済み正式captureのhash不一致".into())}
   if j.status=="completed"{verify_capture(&s,&c,&serde_json::from_slice(&bytes).map_err(|e|e.to_string())?)?;crate::practical::verify_product(j)?;return Ok(())}
  }else if j.status!="failed"||j.active_node_id.as_deref()!=Some("writing")||j.nodes["writing"].status!="failed"{return Err("HTML登録で失敗した元Jobだけを復旧します".into())}
  // Only official read-only RPCs. Never call the engine's normal AI worker.
  let rpc=self.get_rpc()?;
  let thread=rpc.call("thread/read",json!({"threadId":c.thread_id,"includeTurns":false}))?;
  let turns=rpc.call("thread/turns/list",json!({"threadId":c.thread_id,"limit":100,"sortDirection":"asc","itemsView":"summary"}))?;
  let mut items=serde_json::Map::new();for b in &c.turns{items.insert(b.turn_id.clone(),rpc.call("thread/items/list",json!({"threadId":c.thread_id,"turnId":b.turn_id,"limit":100,"sortDirection":"asc"}))?);}
  let capture=json!({"thread":thread,"turns":turns,"items":items});
  let output=verify_capture(&s,&c,&capture)?;
  if self.scrub(&output)!=output{return Err("原finalに変更が必要なため厳密回収を停止しました".into())}
  let bytes=serde_json::to_vec_pretty(&capture).map_err(|e|e.to_string())?;
  let capture_path=Path::new(&j.workspace).join(".recovery").join(format!("official-{}.json",sha(&bytes)));install_exact(&capture_path,&bytes)?;
  let expected_job=sha(&serde_json::to_vec(j).map_err(|e|e.to_string())?);
  // Source freshness after all read-only waits, before any registration/state change.
  verify_capture(&self.snapshot(),&c,&capture)?;
  if j.recovery.is_none(){self.update(|s|{
   let j=s.job.as_mut().ok_or("Jobなし")?;check_contract(j,&c)?;
   if sha(&serde_json::to_vec(j).map_err(|e|e.to_string())?)!=expected_job{return Err("読み取り待機中に元Jobが変更されました".into())}
   j.recovery=Some(Recovery{contract:c.clone(),proof_sha256:sha(&proof),capture_path:capture_path.to_string_lossy().into(),capture_sha256:sha(&bytes),stage:"VERIFIED".into(),started_at:now(),original_failed_run:j.nodes["writing"].clone(),original_failed_at:j.ended_at.clone()});
   j.status="queued".into();j.ended_at=None;j.nodes.get_mut("writing").unwrap().status="queued".into();
   s.event(Some("writing"),"formal_recovery_verified","queued",format!("元の正式2 turnと原final {} bytes / SHA-256 {}を再検証。追加推論0。元の失敗と部分streamを保全。",output.len(),sha(output.as_bytes())),None,vec![capture_path.to_string_lossy().into()]);Ok(())
  })?;}
  self.finish_formal_recovery(&output)
 }
 pub(crate) fn finish_formal_recovery(&self,output:&str)->Result<(),String>{
  let s=self.snapshot();let j=s.job.as_ref().ok_or("Jobなし")?;let r=j.recovery.as_ref().ok_or("正式回収の証拠なし")?;
  if sha(output.as_bytes())!=r.contract.turns[1].response_sha256||output.len() as u64!=r.contract.turns[1].response_bytes{return Err("回収済み原final不一致".into())}
  if ["failed","cancelled"].contains(&j.status.as_str()){self.update(|s|{s.job.as_mut().unwrap().status="queued".into();Ok(())})?;}
  self.update(|s|{s.job.as_mut().unwrap().status="running".into();Ok(())})?;
  for node_id in ["writing","qa","delivery"]{
   let s=self.snapshot();let j=s.job.as_ref().unwrap();let n=j.workflow.nodes.iter().find(|n|n.node_id==node_id).ok_or("元工程なし")?.clone();
   if j.nodes[node_id].status=="completed"{continue}
   crate::practical::required_inputs(j,&n)?;
   if ["failed","cancelled"].contains(&j.nodes[node_id].status.as_str()){self.update(|s|{s.job.as_mut().unwrap().nodes.get_mut(node_id).unwrap().status="queued".into();Ok(())})?;}
   self.update(|s|{let j=s.job.as_mut().unwrap();j.status="running".into();j.ended_at=None;let run=j.nodes.get_mut(node_id).unwrap();run.status="running".into();if run.attempt==0{run.attempt=1;run.started_at=Some(now())}run.ended_at=None;j.active_node_id=Some(node_id.into());j.recovery.as_mut().unwrap().stage=node_id.into();s.event(Some(node_id),"recovery_stage_started","running","正式応答の回収後工程を再開。AI dispatcherは使用しません。",None,vec![]);Ok(())})?;
   match node_id{"writing"=>self.accept_codex_output(&n,n.output_artifacts.first().ok_or("元成果物名なし")?,output)?,"qa"=>self.quality_node(&n)?,"delivery"=>self.save_manifest(&n)?,_=>unreachable!()}
   self.update(|s|{let j=s.job.as_mut().unwrap();let run=j.nodes.get_mut(node_id).unwrap();run.status="completed".into();run.ended_at=Some(now());let paths=j.artifacts.iter().filter(|a|a.node_id==node_id).map(|a|a.path.clone()).collect();s.event(Some(node_id),"node_completed","completed","元成果物の検証・登録を確認。元の失敗は履歴に保持。",None,paths);Ok(())})?;
  }
  let s=self.snapshot();let j=s.job.as_ref().unwrap();crate::practical::verify_product(j)?;
  for a in &j.artifacts{verify_artifact(a,Path::new(&j.workspace))?;}
  if j.nodes.values().any(|r|r.status!="completed")||j.quality.as_ref().is_none_or(|q|q.machine_status!="PASS"){return Err("未完了工程/QAが残っています".into())}
  self.update(|s|{let j=s.job.as_mut().unwrap();if j.status!="completed"{j.status="completed".into();j.active_node_id=None;j.turn_id=None;j.ended_at=Some(now());j.recovery.as_mut().unwrap().stage="COMPLETED".into();s.event(None,"job_completed","completed","同じJobの正式応答を追加推論0で登録・機械QA・保存。実表示/印刷は独立レビューを参照。",None,vec![]);}Ok(())})
 }
}
#[cfg(test)]#[path="recovery_tests.rs"]mod tests;
