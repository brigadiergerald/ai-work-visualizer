use crate::{core_api::{self,Action},engine::Engine,model::*,purposes,rpc::Rpc,setup::Settings};
use serde_json::{json,Value};
use std::{fs,path::PathBuf,sync::{Arc,atomic::Ordering},time::{Instant,Duration}};
fn fixture(label:&str,raw:&str)->(PathBuf,Arc<Engine>){
 let root=PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").unwrap()).join(format!("sns-output-{label}-{}",id()));fs::create_dir_all(root.join("output")).unwrap();fs::write(root.join(".ai-guild-qa"),"OFFLINE only").unwrap();
 let e=Engine::open(&root,None).unwrap();e.save_setup(Settings{work_root:root.to_string_lossy().into(),output_root:root.join("output").to_string_lossy().into(),approval_policy:"on-request".into(),..Default::default()}).unwrap();
 fs::write(root.join("offline-plan.json"),json!({"title":"鶏肉を紹介する文章案","requirements":["提供された鶏肉の特徴を紹介する"],"checklist":["未提供の本人の体験は書かない"],"productionConstraints":["不足する文体・長さはAI判断、一般案内で制作"],"files":[{"name":"post.txt","requiredText":["鶏肉はたんぱく質が豊富で糖質が少ない"]}],"testCommand":""}).to_string()).unwrap();fs::write(root.join("offline-event.html"),raw).unwrap();
 *e.rpc.lock().unwrap()=Some(Rpc::phase9_fixture(&root).unwrap());(root,e)
}
fn wait(e:&Engine){let end=Instant::now()+Duration::from_secs(15);while e.worker.load(Ordering::SeqCst)&&Instant::now()<end{std::thread::sleep(Duration::from_millis(20));}assert!(!e.worker.load(Ordering::SeqCst));}
fn start(e:&Arc<Engine>){core_api::start_prepared_purpose(e,purposes::prepare(purposes::SOCIAL,purposes::PRIOR_VERSION,json!({"theme":"40代の男性に鶏肉の良さを伝える。","context":"Threads、鶏肉はたんぱく質が豊富で糖質が少ない","contentLanguage":"ja"})).unwrap(),None).unwrap();wait(e);}
fn good()->String{json!({"files":[{"name":"post.txt","content":"鶏肉はたんぱく質が豊富で糖質が少ない。日々の食事選びで、こんな特徴にも目を向けてみませんか。"}]}).to_string()}
#[test]fn sns_autonomous_schema_two_turns_without_missing_episode_questions(){
 let(p,e)=fixture("no-episode",&good());start(&e);let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"awaiting_approval");assert_eq!(j.practical.as_ref().unwrap().turn_starts,2);assert_eq!(s.approvals.len(),1);assert_eq!(s.approvals[0].kind,"content_review");assert_eq!(s.purpose_job_context[&j.job_id].definition_version,"1.1.0");
 let rows=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap().lines().map(|s|serde_json::from_str::<Value>(s).unwrap()).collect::<Vec<_>>();assert_eq!(rows.len(),2);for(row,node)in rows.iter().zip(["plan","writing"]){assert_eq!(row["outputSchema"],crate::sns_output::schema(node).unwrap());assert_eq!(row["sandboxPolicy"],crate::rpc::readonly_policy());}
 assert!(!j.workflow.nodes.iter().any(|n|n.work_instruction.contains("本人のエピソードが必要で素材がなければ質問する")));assert!(crate::sns::autonomous(j));crate::general::verify_product(j).unwrap();e.get_rpc().unwrap().shutdown();
}
#[test]fn sns_autonomous_rejects_tool_questions_without_waiting_or_fabricated_answers(){
 let(p,e)=fixture("tool-question",&good());fs::write(p.join("offline-question-request.json"),json!([{"id":"experience","question":"実体験を教えてください","isSecret":false,"options":[]}]).to_string()).unwrap();start(&e);let s=e.snapshot();assert_eq!(s.job.as_ref().unwrap().status,"awaiting_approval");assert!(s.approvals.iter().all(|a|a.kind=="content_review"));assert!(s.events.iter().any(|e|e.event_type=="sns_clarification_not_requested"));
 let rows=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap().lines().map(|s|serde_json::from_str::<Value>(s).unwrap()).collect::<Vec<_>>();let r=rows.iter().find(|r|r["event"]=="question-response").unwrap();assert_eq!(r["response"]["error"]["code"],-32601);assert!(r["response"].get("result").is_none());assert_eq!(rows.iter().filter(|r|r["event"]=="turn/start").count(),2);e.get_rpc().unwrap().shutdown();
}
#[test]fn sns_invalid_question_bundle_remains_failed_and_complete_raw_is_preserved(){
 let raw=json!({"files":[],"question":"確認質問を納品として扱わない。".repeat(100)}).to_string();let(_p,e)=fixture("invalid-question",&raw);start(&e);let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"failed");assert_eq!(j.nodes["writing"].output,raw);assert_eq!(j.practical.as_ref().unwrap().turn_starts,2);assert_eq!(j.nodes["qa"].attempt,0);assert!(j.artifacts.iter().all(|a|a.kind!="product"));assert!(s.approvals.is_empty());assert!(j.nodes["writing"].error.as_ref().unwrap().contains("unknown field"));e.get_rpc().unwrap().shutdown();
}
#[test]fn sns_legacy_purpose_hash_loads_but_stale_start_is_rejected(){
 let input=json!({"theme":"旧Job","contentLanguage":"ja"});let old=purposes::prepare(purposes::SOCIAL,purposes::LEGACY_VERSION,input.clone()).unwrap();assert_eq!(old.context.workflow_sha256,"1e4f9f1768a7376b636c60f605d4cc155520570ea0f6f105a2185e3158a3bca6");let new=purposes::prepare(purposes::SOCIAL,purposes::VERSION,input.clone()).unwrap();assert_ne!(old.context.workflow_sha256,new.context.workflow_sha256);
 let(_p,e)=fixture("stale-start",&good());let before=serde_json::to_value(e.snapshot()).unwrap();let err=core_api::perform(&e,Action::StartPurpose{purpose_id:purposes::SOCIAL.into(),definition_version:purposes::LEGACY_VERSION.into(),input,completion_plan:None}).unwrap_err();assert!(err.starts_with("[PURPOSE_NOT_STARTED]"));assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);e.get_rpc().unwrap().shutdown();
}
