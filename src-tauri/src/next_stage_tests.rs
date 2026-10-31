use crate::{engine::*,model::*,practical::{self,Plan},rpc::Rpc};
use std::{fs,path::PathBuf,sync::{Arc,atomic::Ordering},time::{Instant,Duration}};
use serde_json::json;
fn root()->PathBuf{let p=PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").expect("isolated root required")).join(format!("next-stage-{}",id()));fs::create_dir_all(&p).unwrap();fs::write(p.join(".ai-guild-qa"),"OFFLINE ONLY / INFERENCE 0").unwrap();p}
fn instruction(ext:&str)->String{format!("チーム向けに「朝の確認」を含む引継ぎメモを{ext}で作成してください。手順は共有事項の確認です。")}
fn plan()->Plan{Plan{production_constraints:vec![],title:"朝の確認".into(),requirements:vec!["朝の確認".into()],checklist:vec!["共有事項の確認".into()]}}
fn output(ext:&str)->String{match ext{"HTML"=>"<!doctype html><html lang=\"ja\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>朝の確認</title><style>body{background:#fff;color:#172c3c;font-family:sans-serif;font-size:1rem;line-height:1.8;margin:2rem}</style></head><body><h1>朝の確認</h1><p>OFFLINE FIXTURE — 実推論による完成品ではありません</p><section><h2>共有事項の確認</h2><p>引継ぎ内容を読み、未確認事項を担当者と確認してください。</p></section></body></html>".into(),"JSON"=>"{\"title\":\"朝の確認\",\"task\":\"共有事項の確認\",\"classification\":\"OFFLINE FIXTURE\"}".into(),_=>"朝の確認\n\n共有事項の確認\nOFFLINE FIXTURE — 実推論による完成品ではありません。".into()}}
fn wait(e:&Arc<Engine>){let until=Instant::now()+Duration::from_secs(12);while e.worker.load(Ordering::SeqCst)&&Instant::now()<until{std::thread::sleep(Duration::from_millis(30))}assert!(!e.worker.load(Ordering::SeqCst));}
fn pipeline(ext:&str)->(Arc<Engine>,PathBuf){let p=root();fs::write(p.join("offline-plan.json"),serde_json::to_vec(&plan()).unwrap()).unwrap();fs::write(p.join("offline-event.html"),output(ext)).unwrap();let e=Engine::open(&p,None).unwrap();e.update(|s|{s.connection.version="MOCK / Next-stage offline fixture / NO INFERENCE".into();Ok(())}).unwrap();*e.rpc.lock().unwrap()=Some(Rpc::phase9_fixture(&p).unwrap());e.start(instruction(ext)).unwrap();wait(&e);e.get_rpc().unwrap().shutdown();(e,p)}
#[test]fn next_stage_general_formats_pipeline_restart_history_and_open_boundary(){
 let mut seed=None;
 for ext in ["HTML","Markdown","TXT","JSON"]{
  let(e,p)=pipeline(ext);let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"completed");assert_eq!(j.workflow_id,"codex-document-v1");assert_eq!(j.practical.as_ref().unwrap().turn_starts,2);assert_eq!(j.artifacts.len(),4);
  practical::verify_product(j).unwrap();let file=j.artifacts.iter().find(|a|a.node_id=="writing").unwrap();crate::artifact_open::prepare(&s,&file.path).unwrap();assert_eq!(fs::read_to_string(&file.path).unwrap(),output(ext));
  let rows=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap().lines().map(|x|serde_json::from_str::<serde_json::Value>(x).unwrap()).collect::<Vec<_>>();assert_eq!(rows.len(),2);assert!(rows.iter().all(|x|x["inference"]==false&&x["fullPlanReceived"]==true));
  let identity=j.job_id.clone();let before=serde_json::to_value(j).unwrap();drop(e);let restored=Engine::open(&p,None).unwrap();let read=restored.snapshot();assert_eq!(serde_json::to_value(read.job.as_ref().unwrap()).unwrap(),before);crate::artifact_open::prepare(&read,&file.path).unwrap();
  // An immutable completed job remains available from history after another job.
  restored.update(|s|{let previous=s.job.take().unwrap();s.history.push(previous);Ok(())}).unwrap();crate::artifact_open::prepare(&restored.snapshot(),&file.path).unwrap();assert_eq!(restored.snapshot().history[0].job_id,identity);
  restored.update(|s|{s.job=s.history.pop();Ok(())}).unwrap();
  fs::write(p.join("result.json"),serde_json::to_vec_pretty(&json!({"classification":"OFFLINE_ONLY","jobId":identity,"format":ext,"pipeline":"PASS","restart":"PASS","historicalOpenValidation":"PASS","inference":0,"artifact":file})).unwrap()).unwrap();
  if ext=="TXT"{seed=Some(json!({"dataPath":p,"jobId":identity,"artifact":file}));}
 }
 fs::write(PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").unwrap()).join("native-open-seed.json"),serde_json::to_vec_pretty(&seed.unwrap()).unwrap()).unwrap();
}
#[test]fn next_stage_general_same_job_formal_recovery_without_inference(){
 use crate::recovery::{self,Binding,Contract,sha};
 let p=root();let e=Engine::open(&p,None).unwrap();let input=instruction("TXT");let practical=practical::intake(&input).unwrap().unwrap();let w=practical::workflow_for(&practical);
 let mut j=Job{job_id:id(),workflow_id:w.workflow_id.clone(),nodes:w.nodes.iter().map(|n|(n.node_id.clone(),Run::default())).collect(),workflow:w,status:"running".into(),mode:"real".into(),instruction:input,started_at:now(),ended_at:None,workspace:p.to_string_lossy().into(),thread_id:Some("OFFLINE_GENERAL_RECOVERY".into()),turn_id:None,active_node_id:Some("writing".into()),artifacts:vec![],stop_requested:false,pause_requested:false,practical:Some(practical),quality:None,recovery:None,collaboration:None};
 j.practical.as_mut().unwrap().turn_starts=2;j.nodes.get_mut("writing").unwrap().attempt=1;j.nodes.get_mut("writing").unwrap().status="running".into();
 e.update(|s|{s.job=Some(j);Ok(())}).unwrap();let n=e.snapshot().job.unwrap().workflow.nodes[0].clone();let plan_raw=serde_json::to_string(&plan()).unwrap();e.accept_codex_output(&n,"plan.json",&plan_raw).unwrap();
 e.update(|s|{s.job.as_mut().unwrap().nodes.get_mut("plan").unwrap().status="running".into();Ok(())}).unwrap();e.update(|s|{s.job.as_mut().unwrap().nodes.get_mut("plan").unwrap().status="completed".into();Ok(())}).unwrap();
 let s=e.snapshot();let j=s.job.as_ref().unwrap();let mut bindings=vec![];let mut items=serde_json::Map::new();let mut turns=vec![];
 for(node,raw)in [("plan",plan_raw),("writing",output("TXT"))]{let turn=format!("OFFLINE_{node}");let request=recovery::prompt(&s,j,node).unwrap();bindings.push(Binding{node_id:node.into(),turn_id:turn.clone(),response_sha256:sha(raw.as_bytes()),response_bytes:raw.len() as u64,request_sha256:sha(request.as_bytes())});turns.push(json!({"id":turn,"status":"completed"}));items.insert(turn.clone(),json!({"data":[{"turnId":turn,"item":{"type":"userMessage","content":[{"type":"text","text":request}]}},{"turnId":turn,"item":{"type":"agentMessage","phase":"final_answer","text":raw}}],"nextCursor":null}));}
 let c=Contract{job_id:j.job_id.clone(),thread_id:j.thread_id.clone().unwrap(),turns:bindings};let capture=json!({"thread":{"thread":{"id":c.thread_id,"cwd":p}},"turns":{"data":turns,"nextCursor":null},"items":items});
 fs::write(p.join("proof.json"),serde_json::to_vec(&c).unwrap()).unwrap();fs::write(p.join("offline-recovery-capture.json"),serde_json::to_vec(&capture).unwrap()).unwrap();
 e.update(|s|{let j=s.job.as_mut().unwrap();j.status="failed".into();j.nodes.get_mut("writing").unwrap().status="failed".into();s.event(Some("writing"),"execution_error","failed","OFFLINE injected registration interruption",None,vec![]);Ok(())}).unwrap();
 *e.rpc.lock().unwrap()=Some(Rpc::phase9_fixture(&p).unwrap());e.recover_formal(p.join("proof.json").to_string_lossy().into()).unwrap();let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.job_id,c.job_id);assert_eq!(j.status,"completed");assert_eq!(j.nodes["writing"].attempt,1);assert_eq!(j.practical.as_ref().unwrap().turn_starts,2);assert!(s.events.iter().any(|v|v.message=="OFFLINE injected registration interruption"));
 let records=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap();assert!(!records.contains("turn/start"));e.get_rpc().unwrap().shutdown();drop(e);let restored=Engine::open(&p,None).unwrap();practical::verify_product(restored.snapshot().job.as_ref().unwrap()).unwrap();
 fs::write(p.join("result.json"),serde_json::to_vec_pretty(&json!({"classification":"OFFLINE_RECOVERY","jobId":c.job_id,"sameTask":"writing","sameAttempt":1,"result":"PASS","extraInference":0,"readOnlyRpc":records,"failureHistoryPreserved":true})).unwrap()).unwrap();
}
#[test]fn document_e2e_preflight_routing_and_markdown_without_job_or_inference(){
 let input=include_str!("../../tests/fixtures/document-e2e/input.txt");
 let p=practical::intake(input).unwrap().unwrap();assert_eq!(p.version,4);assert_eq!(p.turn_starts,0);assert_eq!(p.max_turn_starts,2);
 let w=practical::workflow_for(&p);validate(&w).unwrap();assert_eq!(w.workflow_id,"codex-document-v1");assert_eq!(w.nodes.iter().filter(|n|n.executor=="codex").count(),2);assert_eq!(w.nodes[1].output_artifacts,vec!["document.md"]);
 let locks=crate::codex_only::exact_copy(input);assert_eq!(locks.len(),8);
 let plan=Plan{production_constraints:vec![],title:locks[0].clone(),requirements:locks.clone(),checklist:vec![]};
 practical::parse_plan(&serde_json::to_string(&plan).unwrap(),input).unwrap();
 // This is only validator fixture text, never a generated or registered product.
 let raw=locks.iter().map(|s|format!("## {s}\n- [ ] OFFLINE検査用の項目を確認する。\n")).collect::<String>();
 crate::codex_only::inspect(&raw,&plan,input).unwrap();
 assert!(crate::codex_only::inspect(&raw.replace("- [ ] ","- "),&plan,input).is_err());
 for lock in &locks{assert!(crate::codex_only::inspect(&raw.replace(lock,"欠落"),&plan,input).is_err());}
 assert!(practical::intake(&format!("{input}\nただし画像が必須です。")).is_err());
 assert!(practical::intake(&format!("{input}\n画像を生成してください。")).is_err());
 assert!(practical::intake("「文章や画像の内容確認」をするMarkdownを作る").is_err());
}
#[test]fn next_stage_independent_copy_and_unsupported_work_fail_closed(){
 for text in ["画像を含むHTMLを作る","ChatGPTへ委譲する","PDFで提案書を作成","動画を作って","外部調査で比較文書を作成"]{assert!(practical::intake(text).is_err(),"{text}");}
 assert_eq!(practical::intake("画像なし、外部調査なしで引継ぎメモをTXTで作成").unwrap().unwrap().version,4);
 assert!(practical::intake("画像なしの予定を変更し、画像が必須のHTMLを作成").is_err());
 for ext in ["HTML","Markdown","TXT","JSON"]{let input=instruction(ext);let p=practical::intake(&input).unwrap().unwrap();validate(&practical::workflow_for(&p)).unwrap();crate::codex_only::inspect(&output(ext),&plan(),&input).unwrap();assert!(crate::codex_only::inspect(&output(ext).replace("朝の確認","別の題名"),&plan(),&input).is_err());}
 let mut bad=plan();bad.title="別の題名".into();bad.requirements=vec!["別の題名".into()];assert!(practical::parse_plan(&serde_json::to_string(&bad).unwrap(),&instruction("TXT")).is_err());
 assert!(crate::codex_only::inspect("```json\n{}\n```",&plan(),&instruction("JSON")).is_err());assert!(crate::codex_only::inspect("<script>朝の確認 共有事項の確認</script>",&plan(),&instruction("TXT")).is_err());
}
#[test]fn next_stage_open_rejects_missing_tampered_outside_and_executable(){
 let(e,_)=pipeline("TXT");let s=e.snapshot();let a=s.job.as_ref().unwrap().artifacts.iter().find(|a|a.node_id=="writing").unwrap();
 assert!(crate::artifact_open::prepare(&s,"https://example.invalid/file.txt").is_err());
 let outside=root().join("outside.txt");fs::write(&outside,output("TXT")).unwrap();let mut wrong=s.clone();let replacement=artifact(&outside,"writing","text").unwrap();wrong.job.as_mut().unwrap().artifacts.push(replacement.clone());assert!(crate::artifact_open::prepare(&wrong,&replacement.path).is_err());
 fs::write(&a.path,"tamper").unwrap();assert!(crate::artifact_open::prepare(&s,&a.path).is_err());fs::remove_file(&a.path).unwrap();assert!(crate::artifact_open::prepare(&s,&a.path).is_err());
 let exe=PathBuf::from(&s.job.as_ref().unwrap().workspace).join("blocked.exe");fs::write(&exe,b"MZ").unwrap();let executable=artifact(&exe,"writing","text").unwrap();wrong=s.clone();wrong.job.as_mut().unwrap().artifacts.push(executable.clone());assert!(crate::artifact_open::prepare(&wrong,&executable.path).is_err());
 #[cfg(windows)]{
  let workspace=PathBuf::from(&s.job.as_ref().unwrap().workspace);let target=workspace.join("real");let alias=workspace.join("alias");fs::create_dir(&target).unwrap();let data=target.join("data.txt");fs::write(&data,"safe local text").unwrap();
  assert!(std::process::Command::new("cmd.exe").args(["/c","mklink","/J"]).arg(&alias).arg(&target).status().unwrap().success());
  let mut linked=artifact(&data,"plan","text").unwrap();linked.path=alias.join("data.txt").to_string_lossy().into();wrong=s.clone();wrong.job.as_mut().unwrap().artifacts.push(linked.clone());assert!(crate::artifact_open::prepare(&wrong,&linked.path).err().unwrap().contains("reparse"));
 }
}
#[cfg(windows)]
#[test]fn real_document_extended_workspace_keeps_safe_windows_open(){
 let p=root();let mut s:Snapshot=serde_json::from_str(include_str!("../../tests/fixtures/document-e2e/completed-state.json")).unwrap();let j=s.job.as_mut().unwrap();j.job_id=id();j.workspace=p.canonicalize().unwrap().to_string_lossy().into();
 for a in &mut j.artifacts{let target=p.join(&a.name);let source=PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("tests/fixtures/document-e2e/files").join(&a.name);fs::copy(source,&target).unwrap();*a=artifact(&target,&a.node_id,&a.kind).unwrap();}
 let a=j.artifacts.iter().find(|a|a.name=="document.md").unwrap().clone();let(_,_,resolved)=crate::artifact_open::prepare(&s,&a.path).unwrap();assert!(resolved.is_file());assert_eq!(fs::read(&resolved).unwrap(),fs::read(&a.path).unwrap());
 let mut outside=s.clone();let outside_path=p.parent().unwrap().join("outside.json").to_string_lossy().into_owned();outside.job.as_mut().unwrap().artifacts[0].path=outside_path.clone();assert!(crate::artifact_open::prepare(&outside,&outside_path).is_err());
}
