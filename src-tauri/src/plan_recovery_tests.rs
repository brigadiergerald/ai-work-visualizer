use super::*;
use std::path::PathBuf;

const RAW:&str=include_str!("../../tests/fixtures/document-e2e/plan.raw.txt");
const INPUT:&str=include_str!("../../tests/fixtures/document-e2e/input.txt");
fn fixture()->(Arc<Engine>,PathBuf,Snapshot,Contract,Value){
 let root=PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").expect("isolated offline root required")).join(format!("plan-recovery-{}",id()));fs::create_dir_all(&root).unwrap();fs::write(root.join(".ai-guild-qa"),"OFFLINE RECOVERY TEST; INFERENCE 0").unwrap();
 let e=Engine::open(&root,None).unwrap();let mut original:Snapshot=serde_json::from_str(include_str!("../../tests/fixtures/document-e2e/failed-state.json")).unwrap();
 let j=original.job.as_mut().unwrap();let oldid=j.job_id.clone();j.job_id=id();let jobid=j.job_id.clone();j.thread_id=Some("OFFLINE_PLAN_THREAD".into());j.workspace=root.to_string_lossy().into();for event in &mut original.events{if event.job_id==oldid{event.job_id=jobid.clone()}}for i in &mut original.instructions{if i.job_id==oldid{i.job_id=jobid.clone()}}
 original.connection.version="MOCK / OFFLINE PLAN RECOVERY TEST".into();original.project.path=root.to_string_lossy().into();original.data_path=root.to_string_lossy().into();e.update(|s|{*s=original.clone();Ok(())}).unwrap();
 let request=recovery::prompt(&original,original.job.as_ref().unwrap(),"plan").unwrap();let turn="OFFLINE_PLAN_TURN";
 let c=Contract{job_id:jobid,thread_id:"OFFLINE_PLAN_THREAD".into(),turns:vec![recovery::Binding{node_id:"plan".into(),turn_id:turn.into(),response_sha256:sha(RAW.as_bytes()),response_bytes:RAW.len() as u64,request_sha256:sha(request.as_bytes())}]};
 let capture=json!({"thread":{"thread":{"id":c.thread_id,"cwd":root}},"turns":{"data":[{"id":turn,"status":"completed"}],"nextCursor":null},"items":{turn:{"data":[{"turnId":turn,"item":{"type":"userMessage","content":[{"type":"text","text":request}]}},{"turnId":turn,"item":{"type":"agentMessage","phase":"final_answer","text":RAW}}],"nextCursor":null}}});(e,root,original,c,capture)
}
#[test]fn original_title_field_is_valid_without_weakening_final_locks(){
 let plan=crate::practical::parse_plan(RAW,INPUT).unwrap();assert_eq!(plan.title,"制作物の納品前チェックリスト");
 for lock in crate::codex_only::exact_copy(INPUT){let mut bad:Value=serde_json::from_str(RAW).unwrap();bad["title"]=json!("別の題名");for x in bad["requirements"].as_array_mut().unwrap(){if x.as_str().unwrap()==lock{*x=json!("欠落")}}assert!(crate::practical::parse_plan(&bad.to_string(),INPUT).is_err());}
 let body=format!("{}\n{}",plan.requirements.join("\n"),plan.checklist.join("\n"));assert!(crate::codex_only::inspect(&body,&plan,INPUT).is_err());
}
#[test]fn exact_plan_registration_preserves_identity_history_and_zero_inference(){
 let(e,root,original,c,capture)=fixture();register(&e,&original,&c,&capture,"OFFLINE_GRANT").unwrap();let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"paused");assert_eq!(j.job_id,c.job_id);assert_eq!(j.thread_id.as_ref().unwrap(),&c.thread_id);assert_eq!(j.practical.as_ref().unwrap().turn_starts,1);assert_eq!(j.nodes["plan"].attempt,1);assert_eq!(j.nodes["plan"].status,"completed");assert_eq!(j.nodes["writing"].attempt,0);assert_eq!(j.artifacts.len(),1);assert_eq!(fs::read(root.join("plan.json")).unwrap(),RAW.as_bytes());assert!(s.events.iter().any(|e|e.event_type=="execution_error"));assert!(e.rpc.lock().unwrap().is_none());let before=serde_json::to_value(&s).unwrap();register(&e,&original,&c,&capture,"OFFLINE_GRANT").unwrap();assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);drop(e);let restored=Engine::open(&root,None).unwrap();assert_eq!(restored.snapshot().job.unwrap().practical.unwrap().turn_starts,1);
}
#[test]fn plan_recovery_rejects_wrong_source_task_hash_or_missing_lock(){
 let(e,_,original,c,capture)=fixture();for mutation in 0..6{let mut bad=capture.clone();match mutation{0=>bad["thread"]["thread"]["id"]=json!("WRONG"),1=>bad["turns"]["data"][0]["status"]=json!("inProgress"),2=>bad["turns"]["nextCursor"]=json!("MORE"),3=>bad["items"]["OFFLINE_PLAN_TURN"]["data"][0]["item"]["content"][0]["text"]=json!("WRONG"),4=>bad["items"]["OFFLINE_PLAN_TURN"]["data"][1]["item"]["text"]=json!("PARTIAL"),_=>bad["items"]["OFFLINE_PLAN_TURN"]["data"][1]["turnId"]=json!("WRONG")};assert!(verify(&e.snapshot(),&original,&c,&bad).is_err());}
 let mut wrong=c.clone();wrong.job_id="WRONG".into();assert!(verify(&e.snapshot(),&original,&wrong,&capture).is_err());wrong=c.clone();wrong.turns.clear();assert!(verify(&e.snapshot(),&original,&wrong,&capture).is_err());assert!(e.snapshot().job.unwrap().artifacts.is_empty());assert!(e.rpc.lock().unwrap().is_none());
}
#[test]fn consumed_writing_claim_and_source_drift_stop_before_any_rpc(){
 let(e,root,original,c,capture)=fixture();
 let state=root.join("original.json");let proof=root.join("proof.json");let authority=root.join("authority.txt");let criteria=root.join("criteria.json");
 fs::write(&state,serde_json::to_vec(&original).unwrap()).unwrap();fs::write(&proof,serde_json::to_vec(&c).unwrap()).unwrap();fs::write(&authority,"OFFLINE TEST AUTHORITY ONLY; INFERENCE 0").unwrap();fs::write(&criteria,"OFFLINE CRITERIA SOURCE").unwrap();
 let grant=root.join("grant.json");let value=json!({"version":1,"jobId":c.job_id,"threadId":c.thread_id,"originalStatePath":state,"originalStateSha256":sha(&fs::read(&state).unwrap()),"authorityPath":authority,"authoritySha256":sha(&fs::read(&authority).unwrap()),"criteriaPath":criteria,"criteriaSha256":sha(&fs::read(&criteria).unwrap()),"proofPath":proof,"proofSha256":sha(&fs::read(&proof).unwrap()),"candidateSha256":sha(&fs::read(std::env::current_exe().unwrap()).unwrap()),"allowWritingTurn":true,"maxTurnStarts":2});fs::write(&grant,serde_json::to_vec(&value).unwrap()).unwrap();let h=sha(&fs::read(&grant).unwrap());
 register(&e,&original,&c,&capture,&h).unwrap();let claim=root.join(".recovery").join(format!("writing-{h}.claim"));fs::write(&claim,"OFFLINE CONSUMED CLAIM").unwrap();assert!(continue_writing(&e,grant.to_str().unwrap()).unwrap_err().contains("一回限り"));assert!(e.rpc.lock().unwrap().is_none());assert_eq!(e.snapshot().job.as_ref().unwrap().practical.as_ref().unwrap().turn_starts,1);
 fs::write(&criteria,"CHANGED").unwrap();assert!(load(grant.to_str().unwrap()).is_err());assert_eq!(fs::read_to_string(claim).unwrap(),"OFFLINE CONSUMED CLAIM");assert!(e.rpc.lock().unwrap().is_none());
}
