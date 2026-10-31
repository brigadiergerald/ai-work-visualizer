use crate::{engine::Engine,integrations::{self,Action,Goal,PlanInput,Profile,Scenario},model::*,rpc::Rpc,sns};
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf},sync::{Arc,atomic::Ordering},time::{Instant,Duration}};
fn root(label:&str)->PathBuf{let p=PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").expect("isolated test root")).join(format!("optional-{label}-{}",id()));fs::create_dir_all(&p).unwrap();fs::write(p.join(".ai-guild-qa"),"OPTIONAL INTEGRATIONS / OFFLINE ONLY").unwrap();p}
fn profile()->Profile{Profile{id:id(),revision:0,label:"架空の接続先".into(),provider:"custom".into(),transport:"webhook".into(),endpoint:"https://example.invalid/publish".into(),account_label:"@fictional-test-only".into(),credential_ref:Some("my-account-reference".into()),capabilities:vec![Goal::ExternalDraft,Goal::Scheduled,Goal::PublishedVerified]}}
fn call(e:&Engine,a:Action)->Value{integrations::perform(e,a).unwrap()}
fn get(e:&Engine)->Value{call(e,Action::Get{job_id:e.snapshot().job.map(|j|j.job_id)})}
fn wait(e:&Engine){let end=Instant::now()+Duration::from_secs(30);while e.worker.load(Ordering::SeqCst)&&Instant::now()<end{std::thread::sleep(Duration::from_millis(20));}assert!(!e.worker.load(Ordering::SeqCst));}
fn configured(p:&Path)->Arc<Engine>{let e=Engine::open(p,None).unwrap();fs::create_dir_all(p.join("output")).unwrap();e.save_setup(crate::setup::Settings{work_root:p.to_string_lossy().into(),output_root:p.join("output").to_string_lossy().into(),approval_policy:"on-request".into(),..Default::default()}).unwrap();e}
fn complete(e:&Arc<Engine>,p:&Path,plan:Option<PlanInput>)->String{
 fs::write(p.join("offline-plan.json"),json!({"title":"任意接続のオフライン検証","requirements":["架空の図書室"],"checklist":["投稿しない"],"files":[{"name":"post.txt","requiredText":["架空の図書室"]}]}).to_string()).unwrap();
 fs::write(p.join("offline-event.html"),json!({"files":[{"name":"post.txt","content":"【架空テスト】架空の図書室で本の交換会を準備しています。どんな本に出会えるでしょう？\n"}]}).to_string()).unwrap();
 *e.rpc.lock().unwrap()=Some(Rpc::phase9_fixture(p).unwrap());
 let id=e.start_workflow_with_completion("OFFLINE ONLY：架空の図書室のSNS文章。外部投稿なし。".into(),Some(sns::workflow()),plan).unwrap();wait(e);
 let a=e.snapshot().approvals.iter().find(|a|!a.resolved&&a.job_id==id).unwrap().id.clone();e.answer_content_review(&id,&a,"accept").unwrap();wait(e);e.get_rpc().unwrap().shutdown();assert_eq!(e.snapshot().job.unwrap().status,"completed");id
}
#[test]fn optional_profiles_cas_validation_and_mock_never_claim_real_connection(){
 let p=root("settings");let e=Engine::open(&p,None).unwrap();let before=serde_json::to_value(e.snapshot()).unwrap();let mut v=profile();let saved=call(&e,Action::SaveProfile{profile:v.clone()});assert_eq!(saved["profile"]["revision"],1);assert_eq!(saved["connectionStatus"],"NOT_TESTED");
 assert!(integrations::perform(&e,Action::SaveProfile{profile:v.clone()}).is_err());v.revision=1;
 for (scenario,status) in [(Scenario::Success,"PASS"),(Scenario::AuthenticationFailure,"FAILED"),(Scenario::Unsupported,"FAILED"),(Scenario::Timeout,"UNKNOWN"),(Scenario::Processing,"IN_PROGRESS"),(Scenario::WrongAccount,"FAILED")]{let r=call(&e,Action::MockCheck{profile_id:v.id.clone(),profile_revision:1,goal:Goal::ExternalDraft,scenario});assert_eq!(r["status"],status);assert_eq!(r["connectionStatus"],"NOT_TESTED");assert_eq!(r["networkRequests"],0);assert_eq!(r["externalWrites"],0);}
 assert_eq!(get(&e)["checks"].as_array().unwrap().len(),6);assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);
 let mut bad=v.clone();bad.endpoint="https://example.invalid/?token=secret".into();assert!(integrations::perform(&e,Action::SaveProfile{profile:bad}).is_err());
 let mut bad=v.clone();bad.credential_ref=Some("pm_live_secret".into());assert!(integrations::perform(&e,Action::SaveProfile{profile:bad}).is_err());
 assert!(serde_json::from_value::<Action>(json!({"operation":"saveProfile","profile":{"apiKey":"secret"}})).is_err());assert!(serde_json::from_value::<Action>(json!({"operation":"publish","jobId":"anything"})).is_err());
 v.label="改訂した設定".into();call(&e,Action::SaveProfile{profile:v.clone()});assert!(integrations::perform(&e,Action::MockCheck{profile_id:v.id,profile_revision:1,goal:Goal::ExternalDraft,scenario:Scenario::Success}).is_err());
 let saved=get(&e);drop(e);let e=Engine::open(&p,None).unwrap();assert_eq!(get(&e),saved);
}
#[test]fn optional_export_exact_bytes_manifest_dedupe_tamper_and_unchanged_source(){
 let p=root("export");let e=configured(&p);let id=complete(&e,&p,None);let before=serde_json::to_value(e.snapshot()).unwrap();let source=get(&e);let digest=source["sourceDigest"].as_str().unwrap();
 assert!(integrations::perform(&e,Action::Export{job_id:id.clone(),expected_digest:"0".repeat(64)}).is_err());
 let r=call(&e,Action::Export{job_id:id.clone(),expected_digest:digest.into()});let d=&r["delivery"];assert_eq!(d["status"],"EXPORTED");let folder=PathBuf::from(d["folder"].as_str().unwrap());
 let product=e.snapshot().job.unwrap().artifacts.into_iter().find(|a|a.kind=="product").unwrap();assert_eq!(fs::read(folder.join("post.txt")).unwrap(),fs::read(&product.path).unwrap());
 let manifest:Value=serde_json::from_slice(&fs::read(folder.join("delivery-package.json")).unwrap()).unwrap();assert_eq!(manifest["jobId"],id);assert_eq!(manifest["sourceDigest"],digest);assert_eq!(manifest["files"][0]["sha256"],product.sha256);
 assert_eq!(call(&e,Action::Export{job_id:id.clone(),expected_digest:digest.into()}),r);assert_eq!(get(&e)["deliveries"].as_array().unwrap().len(),1);assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);
 let b=fs::read(&product.path).unwrap();fs::write(&product.path,b"tampered").unwrap();assert!(integrations::perform(&e,Action::Export{job_id:id.clone(),expected_digest:digest.into()}).is_err());fs::write(&product.path,b).unwrap();
 fs::write(folder.join("post.txt"),b"tampered output").unwrap();assert!(integrations::perform(&e,Action::VerifyExport{delivery_id:d["id"].as_str().unwrap().into()}).is_err());let s=get(&e);assert_eq!(s["deliveries"][0]["packageIntegrity"],"FAILED");assert_eq!(s["deliveries"][0]["history"].as_array().unwrap().last().unwrap()["result"],"FAILED");assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);
}
#[test]fn optional_completion_goal_is_pinned_before_work_and_export_recovers_without_replay(){
 let p=root("goal-export");let e=configured(&p);let id=complete(&e,&p,Some(PlanInput{goal:Goal::ExportPackage,..Default::default()}));let state=get(&e);assert_eq!(state["goal"],"exportPackage");assert_eq!(state["goalReached"],true);let delivery=state["deliveries"][0]["id"].as_str().unwrap().to_string();
 let before=serde_json::to_value(e.snapshot()).unwrap();e.data.lock().unwrap().store.0.execute("UPDATE delivery_runs SET json=json_set(json,'$.status','WRITING_UNKNOWN') WHERE id=?1",[&delivery]).unwrap();drop(e);let e=Engine::open(&p,None).unwrap();assert_eq!(get(&e)["goalReached"],false);
 assert!(integrations::perform(&e,Action::Export{job_id:id.clone(),expected_digest:state["sourceDigest"].as_str().unwrap().into()}).is_err());call(&e,Action::VerifyExport{delivery_id:delivery});assert_eq!(get(&e)["goalReached"],true);assert_eq!(get(&e)["deliveries"].as_array().unwrap().len(),1);assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);
}
#[test]fn optional_external_goal_stays_unreached_after_mock_pass_and_profile_drift(){
 let p=root("goal-external");let e=configured(&p);let mut v=profile();call(&e,Action::SaveProfile{profile:v.clone()});v.revision=1;
 let id=complete(&e,&p,Some(PlanInput{goal:Goal::Scheduled,profile_id:Some(v.id.clone()),profile_revision:Some(1)}));let before=serde_json::to_value(e.snapshot()).unwrap();let s=get(&e);assert_eq!(s["productionComplete"],true);assert_eq!(s["goalReached"],false);assert_eq!(s["deliveries"][0]["status"],"BLOCKED_UNIMPLEMENTED");
 call(&e,Action::MockCheck{profile_id:v.id.clone(),profile_revision:1,goal:Goal::Scheduled,scenario:Scenario::Success});assert_eq!(get(&e)["goalReached"],false);
 v.label="変更後の接続先".into();call(&e,Action::SaveProfile{profile:v.clone()});assert_eq!(get(&e)["completionPlan"]["profile"]["revision"],1);assert_eq!(get(&e)["completionPlan"]["profile"]["label"],"架空の接続先");
 assert!(integrations::perform(&e,Action::PrepareExternal{job_id:id,expected_digest:s["sourceDigest"].as_str().unwrap().into(),plan:PlanInput{goal:Goal::Scheduled,profile_id:Some(v.id),profile_revision:Some(1)}}).is_err());assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);
}
#[test]fn optional_legacy_snapshot_has_no_implicit_completion_plan(){
 let p=root("legacy");let e=Engine::open(&p,None).unwrap();let v=serde_json::to_value(e.snapshot()).unwrap();assert!(v.get("completionPlans").is_none());let parsed:Snapshot=serde_json::from_value(v.clone()).unwrap();assert_eq!(serde_json::to_value(parsed).unwrap(),v);assert!(e.snapshot().completion_plans.is_empty());
}
#[test]fn optional_native_fixture(){
 let Some(path)=std::env::var_os("AI_GUILD_OPTIONAL_NATIVE_FIXTURE")else{return};let p=PathBuf::from(path);assert!(p.is_absolute()&&!p.exists());fs::create_dir_all(&p).unwrap();fs::write(p.join(".ai-guild-qa"),"OPTIONAL INTEGRATIONS OFFLINE NATIVE FIXTURE").unwrap();let e=configured(&p);complete(&e,&p,Some(PlanInput::default()));fs::write(p.join("fixture-origin.json"),json!({"kind":"OFFLINE_RPC_FIXTURE","realInference":0,"mockTurns":2,"externalRequests":0,"jobId":e.snapshot().job.unwrap().job_id}).to_string()).unwrap();
}
