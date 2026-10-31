use crate::{core_api::{self,Action},engine::Engine,model::*,purposes,rpc::Rpc,setup::Settings};
use serde_json::{json,Value};
use std::{fs,path::PathBuf,sync::{Arc,atomic::Ordering},time::{Duration,Instant}};
fn note(summary:&str)->Value{json!({"summary":summary,"reason":"提供された目的と素材の範囲に合わせた選択。","details":["未提供の本人の経験や数値は加えない。"]})}
fn fixture(with_notes:bool)->(PathBuf,Arc<Engine>){
 let p=PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").unwrap()).join(format!("decisions-{}",id()));fs::create_dir_all(p.join("output")).unwrap();fs::write(p.join(".ai-guild-qa"),"OFFLINE decision fixture").unwrap();
 let e=Engine::open(&p,None).unwrap();e.save_setup(Settings{work_root:p.to_string_lossy().into(),output_root:p.join("output").to_string_lossy().into(),approval_policy:"on-request".into(),..Default::default()}).unwrap();
 fs::write(p.join("offline-research.json"),json!({"topic":"小さく始める","evidenceScope":"PROVIDED_INPUT_ONLY","providedFacts":["原文テーマ"],"candidateAngles":["一歩だけ始める"],"unknowns":[],"limitations":["外部照合なし"]}).to_string()).unwrap();
 let mut plan=json!({"title":"小さく始める","requirements":["短いSNS文"],"checklist":["小さく始めるを含む"],"productionConstraints":["外部照合なし"],"files":[{"name":"post.txt","requiredText":["小さく始める"]}],"testCommand":""});let mut bundle=json!({"files":[{"name":"post.txt","content":"小さく始める。今日は、一歩だけ。何から始めますか？"}]});
 if with_notes{plan["decisionNotes"]=json!({"theme":note("一歩だけ始める切り口を選択。"),"direction":note("SNS読者へ、短い問いから呼びかける。")});bundle["decisionNotes"]=json!({"writing":note("冒頭でテーマ、本文で一歩、最後は問い。")});}
 fs::write(p.join("offline-plan.json"),plan.to_string()).unwrap();fs::write(p.join("offline-event.html"),bundle.to_string()).unwrap();*e.rpc.lock().unwrap()=Some(Rpc::phase9_fixture(&p).unwrap());(p,e)
}
fn start(e:&Arc<Engine>){core_api::perform(e,Action::StartPurpose{purpose_id:purposes::SOCIAL.into(),definition_version:purposes::VERSION.into(),input:json!({"theme":"小さく始める。外部投稿不要。","contentLanguage":"ja"}),completion_plan:None}).unwrap();let end=Instant::now()+Duration::from_secs(25);while e.worker.load(Ordering::SeqCst)&&Instant::now()<end{std::thread::sleep(Duration::from_millis(20));}assert!(!e.worker.load(Ordering::SeqCst));}
#[test]fn decisions_correspond_to_formal_files_tasks_and_survive_restart(){
 let(p,e)=fixture(true);start(&e);let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"completed");assert_eq!(j.practical.as_ref().unwrap().turn_starts,3);let v=crate::decisions::view(j,&s);assert_eq!(v["records"].as_array().unwrap().len(),4);
 for r in v["records"].as_array().unwrap(){assert_eq!(r["status"],"RECORDED");assert_eq!(r["jobId"],j.job_id);assert_eq!(r["attempt"],1);assert!(r["taskId"].is_string());assert!(r["recordedAt"].is_string());let a=j.artifacts.iter().find(|a|r["source"]["path"]==a.path).unwrap();assert_eq!(r["source"]["sha256"],a.sha256);let raw:Value=serde_json::from_slice(&fs::read(&a.path).unwrap()).unwrap();let kind=r["kind"].as_str().unwrap();if kind!="inspection"{assert_eq!(r["note"],raw["decisionNotes"][kind]);}else{assert_eq!(r["inspection"],raw["decisionInspection"]);assert_eq!(r["inspection"]["checks"][3]["result"],"MEASURED_ONLY");assert_eq!(r["inspection"]["unverified"].as_array().unwrap().len(),5);}}
 assert_eq!(v["humanContentApproval"],"NOT_PERFORMED");assert_eq!(v["publication"],"NOT_PERFORMED");fs::write(p.join("decision-sample.json"),serde_json::to_vec_pretty(&v).unwrap()).unwrap();fs::write(p.join("core-completed.json"),serde_json::to_vec_pretty(&core_api::snapshot(&s)).unwrap()).unwrap();fs::write(p.join("presentation-completed.json"),serde_json::to_vec_pretty(&e.presentation_snapshot(&j.job_id).unwrap()).unwrap()).unwrap();e.get_rpc().unwrap().shutdown();drop(e);let e=Engine::open(&p,None).unwrap();let s=e.snapshot();assert_eq!(crate::decisions::view(s.job.as_ref().unwrap(),&s),v);
}
#[test]fn missing_or_malformed_notes_never_fail_production_or_fabricate_reasons(){
 for malformed in [false,true]{let(p,e)=fixture(false);if malformed{let path=p.join("offline-plan.json");let mut v:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();v["decisionNotes"]=json!({"theme":{"summary":"unusable"}});fs::write(path,v.to_string()).unwrap();}start(&e);let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"completed");let v=crate::decisions::view(j,&s);assert_eq!(v["records"][0]["status"],if malformed{"INVALID_RECORD"}else{"NO_RECORD"});assert_eq!(v["records"][1]["note"],Value::Null);assert_eq!(v["records"][2]["note"],Value::Null);e.get_rpc().unwrap().shutdown();}
}
#[test]fn source_tamper_unknown_failure_other_job_and_attempt_do_not_become_success(){
 let(_,e)=fixture(true);start(&e);let mut s=e.snapshot();let id=s.job.as_ref().unwrap().job_id.clone();s.job.as_mut().unwrap().nodes.get_mut("plan").unwrap().status="disconnected".into();let v=crate::decisions::view(s.job.as_ref().unwrap(),&s);assert_eq!(v["records"][0]["stageState"],"disconnected");
 s.job.as_mut().unwrap().nodes.get_mut("plan").unwrap().attempt+=1;let v=crate::decisions::view(s.job.as_ref().unwrap(),&s);assert_eq!(v["records"][0]["note"],Value::Null);s.job.as_mut().unwrap().nodes.get_mut("plan").unwrap().attempt-=1;
 let a=s.job.as_ref().unwrap().artifacts.iter().find(|a|a.name=="plan.json").unwrap();fs::write(&a.path,"tamper").unwrap();let v=crate::decisions::view(s.job.as_ref().unwrap(),&s);assert_eq!(v["records"][0]["status"],"SOURCE_FAILED");assert_eq!(v["records"][0]["note"],Value::Null);
 let mut wrong=s.job.as_ref().unwrap().clone();wrong.job_id="another-job".into();let v=crate::decisions::view(&wrong,&s);assert!(v["records"].as_array().unwrap().iter().all(|r|r["note"].is_null()));assert_eq!(s.job.unwrap().job_id,id);e.get_rpc().unwrap().shutdown();
}
#[test]fn original_four_role_purpose_workflow_hash_is_preserved(){
 let p=purposes::prepare(purposes::SOCIAL,purposes::FOUR_ROLE_VERSION,json!({"theme":"牛のトリビア","context":"","contentLanguage":"ja"})).unwrap();assert_eq!(p.context.workflow_sha256,"5a515a62559dd1c58d4868045000d2853f29028848a613837e6db5ce544a0186");assert_eq!(p.workflow.workflow_id,crate::sns_team::ID);
}

