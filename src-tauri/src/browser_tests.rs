use crate::{browser::{self,Action,Browser,Settings,Target,Platform},engine::Engine,model::*,rpc::Rpc,sns};
use serde_json::{json,Value};
use std::{fs,path::PathBuf,sync::{Arc,atomic::Ordering},time::{Duration,Instant}};
fn root(name:&str)->PathBuf{let p=PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").expect("isolated test root")).join(format!("browser-{name}-{}",id()));fs::create_dir_all(&p).unwrap();fs::write(p.join(".ai-guild-qa"),"OPEN-CORE-08 OFFLINE ONLY").unwrap();p}
fn settings()->Settings {Settings{version:1,revision:0,default_browser:Some(Browser::Chrome),targets:vec![Target{id:"x-test".into(),platform:Platform::X,account_label:"@fictional_open_core_08_x".into(),browser:Browser::Chrome,method:"desktop".into(),account_check:"NOT_VERIFIED".into(),result_state:"NOT_PERFORMED".into()},Target{id:"threads-test".into(),platform:Platform::Threads,account_label:"@fictional_open_core_08_threads".into(),browser:Browser::Iab,method:"desktop".into(),account_check:"NOT_VERIFIED".into(),result_state:"NOT_PERFORMED".into()}]}}
fn call(e:&Engine,a:Action)->Value{browser::perform(e,a).unwrap()}
fn wait(e:&Engine){let end=Instant::now()+Duration::from_secs(30);while e.worker.load(Ordering::SeqCst)&&Instant::now()<end{std::thread::sleep(Duration::from_millis(20));}assert!(!e.worker.load(Ordering::SeqCst));}
fn complete(p:&std::path::Path)->Arc<Engine>{
 fs::create_dir_all(p.join("output")).unwrap();let e=Engine::open(p,None).unwrap();
 e.save_setup(crate::setup::Settings{work_root:p.to_string_lossy().into(),output_root:p.join("output").to_string_lossy().into(),approval_policy:"on-request".into(),..Default::default()}).unwrap();
 fs::write(p.join("offline-plan.json"),json!({"title":"OFFLINE架空SNS文章","requirements":["架空の図書室の案内"],"checklist":["外部公開しない"],"files":[{"name":"post.txt","requiredText":["架空の図書室"]}]}).to_string()).unwrap();
 fs::write(p.join("offline-event.html"),json!({"files":[{"name":"post.txt","content":"【テスト専用・架空SNS文章】架空の図書室で、本の交換会を開く予定です。どんな一冊に出会えるでしょう？"}]}).to_string()).unwrap();
 *e.rpc.lock().unwrap()=Some(Rpc::phase9_fixture(p).unwrap());
 let id=e.start_workflow("OFFLINE TEST ONLY：架空の図書室のSNS文章をpost.txtにする。実体験は不要。外部投稿なし。".into(),Some(sns::workflow())).unwrap();wait(&e);
 let a=e.snapshot().approvals.iter().find(|a|a.job_id==id&&!a.resolved).unwrap().id.clone();e.answer_content_review(&id,&a,"accept").unwrap();wait(&e);
 assert_eq!(e.snapshot().job.as_ref().unwrap().status,"completed");e.get_rpc().unwrap().shutdown();e
}
fn prepare(e:&Engine,target:&str)->Value {let s=e.snapshot();let j=s.job.as_ref().unwrap();let hash=j.artifacts.iter().find(|a|a.name=="post.txt").unwrap().sha256.clone();call(e,Action::Prepare{job_id:j.job_id.clone(),target_id:target.into(),text_sha256:hash,settings_revision:1})}
#[test]fn open_core_browser_settings_cas_restore_and_capabilities(){
 let p=root("settings");let e=Engine::open(&p,None).unwrap();assert!(call(&e,Action::Get)["settings"]["defaultBrowser"].is_null());let original=serde_json::to_value(e.snapshot()).unwrap();
 let v=call(&e,Action::Save{settings:settings()});assert_eq!(v["settings"]["revision"],1);assert!(browser::perform(&e,Action::Save{settings:settings()}).is_err());
 assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),original);drop(e);let e=Engine::open(&p,None).unwrap();assert_eq!(call(&e,Action::Get)["settings"],v["settings"]);
 for b in [Browser::Iab,Browser::Chrome,Browser::Edge,Browser::Brave,Browser::Opera,Browser::Vivaldi,Browser::System]{let c=browser::capability(b,true);assert_eq!(c["connection"],"NOT_VERIFIED");assert_eq!(c["snsLogin"],"NOT_VERIFIED");assert_eq!(c["coreDirectBrowser"],"NOT_SUPPORTED");if !matches!(b,Browser::Iab|Browser::Chrome){assert_eq!(c["availability"],"MANUAL_ONLY")}}
 assert_eq!(browser::capability(Browser::Chrome,false)["availability"],"USER_ACTION_REQUIRED");
 let mut bad=settings();bad.revision=1;bad.targets[0].browser=Browser::Edge;assert!(browser::perform(&e,Action::Save{settings:bad}).is_err());
 let mut spoof=settings();spoof.revision=1;spoof.targets[0].account_check="VERIFIED".into();spoof.targets[0].result_state="PUBLISHED_VERIFIED".into();assert!(browser::perform(&e,Action::Save{settings:spoof}).is_err());
 assert!(serde_json::from_value::<Action>(json!({"operation":"publish","handoffId":"forbidden"})).is_err());
 assert!(serde_json::from_value::<Action>(json!({"operation":"save","settings":{"version":1,"revision":1,"defaultBrowser":"chrome","targets":[],"password":"forbidden"}})).is_err());
}
#[test]fn open_core_browser_handoff_binding_dedup_manual_tamper_and_restart(){
 let p=root("handoff");let e=complete(&p);call(&e,Action::Save{settings:settings()});let before=serde_json::to_value(e.snapshot()).unwrap();let h=prepare(&e,"x-test");let id=h["handoffId"].as_str().unwrap();assert_eq!(prepare(&e,"x-test")["handoffId"],id);let i=prepare(&e,"threads-test");assert_ne!(i["handoffId"],id);
 let copy=call(&e,Action::CopyInstructions{handoff_id:id.into()});assert!(copy["text"].as_str().unwrap().contains("@Chrome"));assert!(copy["text"].as_str().unwrap().contains("NOT_GRANTED"));assert_eq!(call(&e,Action::Get)["handoffs"].as_array().unwrap().len(),2);
 assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);let file=p.join("browser-handoffs").join(id).join("handoff.json");let bytes=fs::read(&file).unwrap();let value:Value=serde_json::from_slice(&bytes).unwrap();assert_eq!(value["publicationApproval"]["textSha256"],value["textSha256"]);assert_eq!(value["publicationApproval"]["accountLabel"],"@fictional_open_core_08_x");assert_eq!(value["accountCheck"],"NOT_VERIFIED");
 // Local file and DB must independently agree; a changed file cannot be copied/opened.
 fs::write(&file,b"tampered").unwrap();assert!(browser::perform(&e,Action::CopyInstructions{handoff_id:id.into()}).is_err());fs::write(&file,bytes).unwrap();
 // An UNKNOWN launch intent is never reused, including after restart.
 e.data.lock().unwrap().store.0.execute("UPDATE browser_handoffs SET status='DESKTOP_OPEN_REQUESTED_UNKNOWN' WHERE id=?1",[id]).unwrap();assert!(browser::perform(&e,Action::OpenDesktop{handoff_id:id.into()}).is_err());let state=call(&e,Action::Get);drop(e);let e=Engine::open(&p,None).unwrap();assert_eq!(call(&e,Action::Get),state);assert!(browser::perform(&e,Action::OpenDesktop{handoff_id:id.into()}).is_err());
 let h:browser::Handoff=serde_json::from_value(value.clone()).unwrap();let url=browser::handoff_link(&h,&p).unwrap();assert!(url.starts_with("codex://new?prompt="));let parsed=tauri::Url::parse(&url).unwrap();let query=parsed.query_pairs().collect::<std::collections::HashMap<_,_>>();assert_eq!(query["path"],file.parent().unwrap().to_string_lossy());assert!(query["prompt"].starts_with("@Chrome"));assert!(query["prompt"].contains(&h.text_sha256));assert!(!query["prompt"].contains(&h.text));assert!(!query.contains_key("send"));
 // Saved settings drift invalidates the previous preparation without mutating it.
 let mut s=settings();s.revision=1;s.targets[0].account_label="@changed".into();call(&e,Action::Save{settings:s});assert!(browser::perform(&e,Action::CopyInstructions{handoff_id:id.into()}).is_err());
 fs::write(p.join("browser-result.json"),serde_json::to_vec_pretty(&json!({"classification":"OFFLINE_PASS","jobStatePreserved":true,"handoffs":state,"newInference":0,"publication":0,"desktopOpen":0,"browserOperations":0})).unwrap()).unwrap();
}
#[test]fn open_core_browser_rejects_unapproved_wrong_job_and_hash(){
 let p=root("negative");let e=Engine::open(&p,None).unwrap();call(&e,Action::Save{settings:settings()});assert!(browser::perform(&e,Action::Prepare{job_id:"wrong".into(),target_id:"x-test".into(),text_sha256:"x".repeat(64),settings_revision:1}).is_err());drop(e);
 let p=root("wrong-hash");let e=complete(&p);call(&e,Action::Save{settings:settings()});let id=e.snapshot().job.unwrap().job_id;assert!(browser::perform(&e,Action::Prepare{job_id:id,target_id:"x-test".into(),text_sha256:"0".repeat(64),settings_revision:1}).is_err());
 let mut s=settings();s.revision=1;s.targets[0].browser=Browser::Vivaldi;s.targets[0].method="manual".into();call(&e,Action::Save{settings:s});let snap=e.snapshot();let j=snap.job.unwrap();let sha=j.artifacts.iter().find(|a|a.name=="post.txt").unwrap().sha256.clone();let h=call(&e,Action::Prepare{job_id:j.job_id,target_id:"x-test".into(),text_sha256:sha,settings_revision:2});let id=h["handoffId"].as_str().unwrap();assert!(browser::perform(&e,Action::OpenDesktop{handoff_id:id.into()}).is_err());let copy=call(&e,Action::CopyInstructions{handoff_id:id.into()});assert!(copy["text"].as_str().unwrap().contains("Vivaldi"));assert!(!copy["text"].as_str().unwrap().contains("@Chrome"));
}
#[test]fn open_core_browser_native_fixture(){
 let Some(p)=std::env::var_os("AI_GUILD_BROWSER_NATIVE_FIXTURE") else{return};let p=PathBuf::from(p);assert!(p.is_absolute()&&!p.exists());fs::create_dir_all(&p).unwrap();fs::write(p.join(".ai-guild-qa"),"OPEN-CORE-08 OFFLINE ONLY / ZERO REAL INFERENCE").unwrap();let e=complete(&p);assert_eq!(e.snapshot().job.as_ref().unwrap().practical.as_ref().unwrap().turn_starts,2);fs::write(p.join("fixture-origin.json"),json!({"classification":"OFFLINE_RPC_FIXTURE","realInference":0,"mockTurnStarts":2,"jobId":e.snapshot().job.unwrap().job_id,"publication":0}).to_string()).unwrap();
}

#[test]fn open_core_browser_explicit_redisplay_preserves_original_and_records_each_os_request(){
 let p=root("redisplay");let e=complete(&p);call(&e,Action::Save{settings:settings()});let h=prepare(&e,"x-test");let id=h["handoffId"].as_str().unwrap().to_string();
 let file=p.join("browser-handoffs").join(&id).join("handoff.json");let bytes=fs::read(&file).unwrap();let before=serde_json::to_value(e.snapshot()).unwrap();
 let urls=std::cell::RefCell::new(Vec::new());
 let opener=|url:&str|{urls.borrow_mut().push(url.to_string());Ok(())};
 let open=browser::perform_with_opener(&e,Action::OpenDesktop{handoff_id:id.clone()},&opener).unwrap();
 assert_eq!(open["displayRequest"]["kind"],"INITIAL");assert_eq!(open["displayRequest"]["osStatus"],"OS_ACCEPTED");assert_eq!(open["displayRequest"]["composerStatus"],"NOT_VERIFIED");
 let original:Vec<String>=e.data.lock().unwrap().store.0.query_row("SELECT json,status,history,file_sha256 FROM browser_handoffs WHERE id=?1",[&id],|r|Ok(vec![r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?])).unwrap();
 assert!(browser::perform_with_opener(&e,Action::OpenDesktop{handoff_id:id.clone()},&opener).is_err());
 for _ in 0..2{
  let r=browser::perform_with_opener(&e,Action::RedisplayDesktop{handoff_id:id.clone()},&opener).unwrap();
  assert_eq!(r["displayRequest"]["kind"],"REDISPLAY");assert_eq!(r["displayRequest"]["automaticSend"],false);
  assert_eq!(r["displayRequest"]["jobId"],before["job"]["jobId"]);assert_eq!(r["displayRequest"]["handoffId"],id);
 }
 assert_eq!(urls.borrow().len(),3);assert!(urls.borrow().iter().all(|u|u==&urls.borrow()[0]));
 let after:Vec<String>=e.data.lock().unwrap().store.0.query_row("SELECT json,status,history,file_sha256 FROM browser_handoffs WHERE id=?1",[&id],|r|Ok(vec![r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?])).unwrap();assert_eq!(original,after);
 assert_eq!(fs::read(&file).unwrap(),bytes);assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);
 let state=call(&e,Action::Get);assert_eq!(state["handoffs"][0]["desktopDisplays"].as_array().unwrap().len(),3);drop(e);let e=Engine::open(&p,None).unwrap();assert_eq!(call(&e,Action::Get),state);
 // Corruption and setting drift must stop before the OS boundary, even for redisplay.
 fs::write(&file,b"tamper").unwrap();assert!(browser::perform_with_opener(&e,Action::RedisplayDesktop{handoff_id:id.clone()},&opener).is_err());fs::write(&file,bytes).unwrap();
 let mut s=settings();s.revision=1;s.targets[0].account_label="@changed".into();call(&e,Action::Save{settings:s});assert!(browser::perform_with_opener(&e,Action::RedisplayDesktop{handoff_id:id.clone()},&opener).is_err());assert_eq!(urls.borrow().len(),3);
}

#[test]fn open_core_browser_unknown_display_stops_replay_and_rejection_remains_separate(){
 let p=root("redisplay-unknown");let e=complete(&p);call(&e,Action::Save{settings:settings()});let h=prepare(&e,"x-test");let id=h["handoffId"].as_str().unwrap().to_string();
 let reject=|_:&str|Err("OFFLINE_OS_REJECTION".to_string());
 assert!(browser::perform_with_opener(&e,Action::OpenDesktop{handoff_id:id.clone()},&reject).is_err());
 let original=call(&e,Action::Get)["handoffs"][0].clone();
 assert!(browser::perform_with_opener(&e,Action::RedisplayDesktop{handoff_id:id.clone()},&reject).is_err());
 let state=call(&e,Action::Get);assert_eq!(state["handoffs"][0]["status"],original["status"]);assert_eq!(state["handoffs"][0]["history"],original["history"]);assert_eq!(state["handoffs"][0]["desktopDisplays"][1]["osStatus"],"OS_REJECTED");
 e.data.lock().unwrap().store.0.execute("UPDATE browser_desktop_displays SET json=json_set(json,'$.osStatus','REQUESTED_UNKNOWN') WHERE handoff_id=?1",[&id]).unwrap();
 let never=|_:&str|->Result<(),String>{panic!("UNKNOWN must never cross OS boundary")};
 assert!(browser::perform_with_opener(&e,Action::RedisplayDesktop{handoff_id:id.clone()},&never).is_err());drop(e);let e=Engine::open(&p,None).unwrap();assert!(browser::perform_with_opener(&e,Action::RedisplayDesktop{handoff_id:id.clone()},&never).is_err());
 e.data.lock().unwrap().store.0.execute("UPDATE browser_handoffs SET status='DESKTOP_OPEN_REQUESTED_UNKNOWN' WHERE id=?1",[&id]).unwrap();assert!(browser::perform_with_opener(&e,Action::RedisplayDesktop{handoff_id:id.clone()},&never).is_err());
 assert!(call(&e,Action::CopyInstructions{handoff_id:id})["text"].as_str().unwrap().contains("@Chrome"));
}

#[test]fn open_core_browser_deep_link_utf8_roundtrip_and_directory_validation(){
 let p=root("encoding 日本語 &+#");let prompt="@Chrome 日本語 & path=bad? + # %20 / \\ \" 😀\n次の行";
 let url=browser::deep_link(prompt,&p).unwrap();let parsed=tauri::Url::parse(&url).unwrap();let q=parsed.query_pairs().collect::<std::collections::HashMap<_,_>>();
 assert_eq!(q.len(),2);assert_eq!(q["prompt"],prompt);assert_eq!(q["path"],p.to_string_lossy());
 assert!(browser::deep_link("test",std::path::Path::new("relative")).is_err());assert!(browser::deep_link("test",&p.join("missing")).is_err());fs::write(p.join("file"),"x").unwrap();assert!(browser::deep_link("test",&p.join("file")).is_err());
}
