use crate::{core_api::{self,Action},engine::Engine,model::*,rpc::Rpc,setup::Settings,sns};
use serde_json::{json,Value};
use std::{path::{Path,PathBuf},fs,sync::{Arc,atomic::Ordering},time::{Instant,Duration}};
fn wait(e:&Engine){let end=Instant::now()+Duration::from_secs(20);while e.worker.load(Ordering::SeqCst)&&Instant::now()<end{std::thread::sleep(Duration::from_millis(20));}assert!(!e.worker.load(Ordering::SeqCst),"OFFLINE peer timeout");}
fn fixture(label:&str)->(PathBuf,Arc<Engine>){
 let p=PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").expect("isolated test root")).join(format!("open-core-{label}-{}",id()));fs::create_dir_all(p.join("output")).unwrap();fs::write(p.join(".ai-guild-qa"),"OFFLINE only").unwrap();
 let e=Engine::open(&p,None).unwrap();e.save_setup(Settings{work_root:p.to_string_lossy().into(),output_root:p.join("output").to_string_lossy().into(),approval_policy:"on-request".into(),..Default::default()}).unwrap();
 fs::write(p.join("offline-plan.json"),json!({"title":"SNS文章案","requirements":["作業を小さく始めることを伝える"],"checklist":["事実を創作しない"],"files":[{"name":"post.txt","requiredText":["小さく始める"]}]}).to_string()).unwrap();
 fs::write(p.join("offline-event.html"),json!({"files":[{"name":"post.txt","content":"まあ、まずは小さく始める。\n昨日はメモを一行だけ書いた、と共有してくれましたね。そこからで十分です。\n今日はどんな一行にしますか？\n"}]}).to_string()).unwrap();
 *e.rpc.lock().unwrap()=Some(Rpc::phase9_fixture(&p).unwrap());(p,e)
}
fn start(e:&Arc<Engine>)->String{let r=core_api::perform(e,Action::Start{completion_plan:None,workflow_id:sns::ID.into(),instruction:"Threads向けに、小さく始める大切さを文章案にしてください。私が昨日メモを一行だけ書いた実体験を入れてください。外部投稿は不要です。".into()}).unwrap();wait(e);r["jobId"].as_str().unwrap().into()}
fn read(e:&Arc<Engine>,id:&str,purpose:&str)->Result<Value,String>{let s=e.snapshot();let j=s.job.as_ref().unwrap();let f=j.artifacts.iter().find(|a|a.name=="post.txt").unwrap();core_api::perform(e,Action::ReadText{job_id:id.into(),path:f.path.clone(),purpose:purpose.into()})}
fn output(p:&Path,name:&str,v:&Value){fs::write(p.join(name),serde_json::to_vec_pretty(v).unwrap()).unwrap();}
#[test]fn open_core_standard_readonly_actual_wire_and_two_turn_budget(){
 let(p,e)=fixture("readonly-wire");start(&e);let calls=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap();let turns=calls.lines().map(|s|serde_json::from_str::<Value>(s).unwrap()).filter(|v|v["event"]=="turn/start").collect::<Vec<_>>();
 assert_eq!(turns.len(),2);for turn in turns{assert_eq!(turn["sandboxPolicy"],crate::rpc::readonly_policy());assert!(turn["sandboxPolicy"].get("access").is_none());}
 assert_eq!(e.snapshot().job.unwrap().status,"awaiting_approval");e.get_rpc().unwrap().shutdown();
}
#[test]fn open_core_sns_approval_save_copy_restart_and_boundary(){
 let (p,e)=fixture("approved");let id=start(&e);let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"awaiting_approval");assert_eq!(j.practical.as_ref().unwrap().turn_starts,2);assert!(j.artifacts.iter().all(|a|a.name!="manifest.json"));assert!(read(&e,&id,"copy").is_err());let text=read(&e,&id,"preview").unwrap();let a=s.approvals.iter().find(|a|!a.resolved).unwrap();
 let view=core_api::snapshot(&s);assert_eq!(view["jobs"][0]["state"],"awaiting_approval");let wire=view.to_string();for forbidden in ["requestId","threadId","turnId","requestUserInput","spriteKey","workInstruction"]{assert!(!wire.contains(forbidden),"{forbidden}")}
 output(&p,"awaiting.json",&view);
 e.answer_content_review(&id,&a.id,"accept").unwrap();wait(&e);assert!(e.answer_content_review(&id,&a.id,"accept").is_err());let result=e.snapshot();let j=result.job.as_ref().unwrap();assert_eq!(j.status,"completed");sns::approved(j).unwrap();crate::general::verify_product(j).unwrap();assert_eq!(text,read(&e,&id,"copy").unwrap());assert!(j.artifacts.iter().any(|a|a.name=="manifest.json"));
 let data=serde_json::to_value((&result.job,&result.history,&result.events,&result.approvals,&result.settings)).unwrap();let complete=core_api::snapshot(&result);output(&p,"completed.json",&complete);
 e.get_rpc().unwrap().shutdown();let calls=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap();assert_eq!(calls.lines().filter(|l|l.contains("\"event\": \"turn/start\"")).count(),2);drop(e);let restored=Engine::open(&p,None).unwrap();let rs=restored.snapshot();assert_eq!(serde_json::to_value((&rs.job,&rs.history,&rs.events,&rs.approvals,&rs.settings)).unwrap(),data);
 // Export into this isolated test run. Never overwrite shared historical UI fixtures.
 let dir=p.join("ui-fixtures");fs::create_dir_all(&dir).unwrap();output(&dir,"open-core.json",&json!({"classification":"OFFLINE_FIXTURE","awaiting":view,"completed":complete,"text":text}));
 output(&p,"result.json",&json!({"result":"OFFLINE_PASS","humanApproval":"digest-bound","restart":"unchanged","liveInference":0,"publication":"NOT_PERFORMED"}));
}
#[test]fn open_core_review_restart_recovery_and_decline_no_inference(){
 let(p,e)=fixture("recover");let id=start(&e);e.get_rpc().unwrap().shutdown();let a=e.snapshot().approvals[0].id.clone();drop(e);let e=Engine::open(&p,None).unwrap();assert_eq!(e.snapshot().job.unwrap().status,"disconnected");core_api::perform(&e,Action::Control{job_id:id.clone(),operation:"recoverReview".into()}).unwrap();assert_eq!(e.snapshot().approvals.len(),1);e.answer_content_review(&id,&a,"decline").unwrap();assert_eq!(e.snapshot().job.unwrap().status,"cancelled");assert!(read(&e,&id,"copy").is_err());assert!(e.snapshot().events.iter().any(|e|e.event_type=="restart_recovery"));assert!(e.snapshot().job.unwrap().artifacts.iter().all(|a|a.name!="manifest.json"));
 output(&p,"result.json",&json!({"result":"OFFLINE_PASS","restartHistoryRetained":true,"duplicateApprovals":0,"replayedInference":0,"declineStopsDelivery":true}));
}
#[test]fn open_core_tamper_wrong_job_and_stop_fail_closed(){
 let(p,e)=fixture("tamper");let id=start(&e);let s=e.snapshot();let a=&s.approvals[0];assert!(core_api::perform(&e,Action::Answer{job_id:"wrong".into(),approval_id:a.id.clone(),decision:"accept".into(),text:String::new(),answers:None}).is_err());
 let f=s.job.as_ref().unwrap().artifacts.iter().find(|a|a.name=="post.txt").unwrap();fs::write(&f.path,"tampered").unwrap();assert!(read(&e,&id,"preview").is_err());assert!(e.answer_content_review(&id,&a.id,"accept").is_err());assert!(!e.snapshot().approvals[0].resolved);assert_eq!(core_api::snapshot(&e.snapshot())["jobs"][0]["artifacts"].as_array().unwrap().iter().find(|a|a["name"]=="post.txt").unwrap()["integrity"],"failed");
 core_api::perform(&e,Action::Control{job_id:id,operation:"stop".into()}).unwrap();assert_eq!(e.snapshot().job.unwrap().status,"cancelled");e.get_rpc().unwrap().shutdown();output(&p,"result.json",&json!({"result":"OFFLINE_PASS","tamper":"rejected","wrongJob":"rejected","stopped":true,"liveInference":0}));
}
#[test]fn open_core_selected_sns_keeps_generic_policy_and_legacy_stop_entry(){
 let(p,e)=fixture("explicit-workflow");let id=e.start_workflow("森の図書室のSNS文章案。小さく始める内容。昨日メモを一行書いた私の体験を使う。投稿不要。".into(),Some(sns::workflow())).unwrap();wait(&e);let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.practical.as_ref().unwrap().version,5);assert_eq!(j.workflow_id,crate::general::ID);assert_eq!(j.status,"awaiting_approval");e.control("stop".into()).unwrap();assert_eq!(e.snapshot().job.unwrap().status,"cancelled");e.get_rpc().unwrap().shutdown();output(&p,"result.json",&json!({"result":"OFFLINE_PASS","jobId":id,"explicitWorkflowWins":true,"legacyControlStop":"PASS","liveInference":0}));
}
