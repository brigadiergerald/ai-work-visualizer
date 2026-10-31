use crate::{engine::*,model::*};
use std::{fs,path::PathBuf,time::{Duration,Instant},sync::atomic::Ordering};
fn test_root(name:&str)->PathBuf{let base=std::env::var_os("AI_GUILD_TEST_ROOT").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("target/test-data"));fs::create_dir_all(&base).unwrap();fs::write(base.join(".ai-guild-qa"),"isolated regression data; no inference").unwrap();let p=base.join(format!("{name}-{}",id()));fs::create_dir_all(&p).unwrap();fs::write(p.join(".ai-guild-qa"),"isolated regression data").unwrap();p}
fn wait(e:&Engine,seconds:u64){let until=Instant::now()+Duration::from_secs(seconds);while e.worker.load(Ordering::SeqCst)&&Instant::now()<until{std::thread::sleep(Duration::from_millis(80))}assert!(!e.worker.load(Ordering::SeqCst),"worker exceeded bounded test wait");}
#[test]fn demo_lifecycle_persistence_and_artifact_integrity(){
 let path=test_root("demo-test");let e=Engine::open(&path,None).unwrap();e.update(|s|{s.mode="demo".into();Ok(())}).unwrap();
 e.start("これはデモ専用の検証です。".into()).unwrap();wait(&e,25);let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"completed");assert_eq!(j.artifacts.len(),5);assert_eq!(s.events.iter().filter(|x|x.event_type=="handoff_started").count(),4);
 for a in &j.artifacts{verify_artifact(a,std::path::Path::new(&j.workspace)).unwrap();assert!(fs::read_to_string(&a.path).unwrap().contains("デモ"));}
 let event_count=s.events.len();let job_id=j.job_id.clone();drop(e);let restored=Engine::open(&path,None).unwrap();let s=restored.snapshot();assert_eq!(s.job.unwrap().job_id,job_id);assert_eq!(s.events.len(),event_count);
 let a=&j.artifacts[0];fs::write(&a.path,"changed test artifact").unwrap();assert!(verify_artifact(a,std::path::Path::new(&j.workspace)).is_err());
 fs::write(path.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"PASS","jobId":job_id,"checks":["5 demo stages","4 actual host handoffs","persistent events","SHA-256 tamper rejection"],"aiInference":false})).unwrap()).unwrap();
}
#[test]fn interruption_pause_retry_and_state_rollback(){
 let path=test_root("control-test");let e=Engine::open(&path,None).unwrap();e.update(|s|{s.mode="demo".into();Ok(())}).unwrap();e.start("中断・再試行の検証".into()).unwrap();
 std::thread::sleep(Duration::from_millis(250));e.control("pause".into()).unwrap();wait(&e,5);assert_eq!(e.snapshot().job.as_ref().unwrap().status,"paused");
 e.control("resume".into()).unwrap();std::thread::sleep(Duration::from_millis(250));e.control("stop".into()).unwrap();wait(&e,5);assert_eq!(e.snapshot().job.as_ref().unwrap().status,"cancelled");
 let before=e.snapshot().events.len();assert!(e.update(|s|{s.job.as_mut().unwrap().status="completed".into();Ok(())}).is_err());assert_eq!(e.snapshot().job.unwrap().status,"cancelled");assert_eq!(e.snapshot().events.len(),before);
 e.control("retry".into()).unwrap();wait(&e,25);assert_eq!(e.snapshot().job.unwrap().status,"completed");
}
#[test]fn restart_retains_unknown_and_never_resends(){
 let p=test_root("restart-test");let e=Engine::open(&p,None).unwrap();e.update(|s|{s.mode="demo".into();Ok(())}).unwrap();e.start("復元確認".into()).unwrap();e.control("pause".into()).unwrap();wait(&e,5);
 e.update(|s|{let j=s.job.as_mut().unwrap();j.status="running".into();j.turn_id=Some("known-turn-for-restart-test".into());Ok(())}).unwrap();drop(e);
 let e=Engine::open(&p,None).unwrap();let s=e.snapshot();let j=s.job.unwrap();assert_eq!(j.status,"disconnected");assert_eq!(j.turn_id.as_deref(),Some("known-turn-for-restart-test"));assert!(e.control("retry".into()).is_err());assert!(s.events.iter().any(|e|e.event_type=="restart_recovery"));
}
#[test]fn redacts_secrets_in_logs(){let secret="Bearer abcde123.abcdef access_token=never_print sk-abcdefghijklmnop";let s=redact(secret);assert!(!s.contains("abcde123"));assert!(!s.contains("never_print"));assert!(!s.contains("sk-abcdefghijklmnop"));}
#[test]fn preserves_per_question_answers_and_omits_secret_history(){
 let params=serde_json::json!({"questions":[{"id":"format","question":"形式は？","isSecret":false},{"id":"secret","question":"秘密の回答","isSecret":true}]});
 assert!(build_user_answers(&params,"same for both",None).is_err());
 let answers=std::collections::HashMap::from([("format".into(),vec!["Markdown".into()]),("secret".into(),vec!["private_value_123".into()])]);
 let(result,history)=build_user_answers(&params,"",Some(answers)).unwrap();
 assert_eq!(result["answers"]["format"]["answers"][0],"Markdown");assert_eq!(result["answers"]["secret"]["answers"][0],"private_value_123");assert!(!history.contains("private_value_123"));assert!(history.contains("Markdown"));
}
#[test]fn sprite_files_decode_with_exact_cells_and_manifest_hashes(){
 let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();let manifest:serde_json::Value=serde_json::from_slice(&fs::read(root.join("public/sprites/manifest.json")).unwrap()).unwrap();
 for c in manifest["characters"].as_array().unwrap(){let p=root.join(c["file"].as_str().unwrap());let decoded=image::open(&p).unwrap();assert_eq!((decoded.width(),decoded.height()),(128,288));let rgba=decoded.to_rgba8();assert_eq!(rgba.get_pixel(0,0).0[3],0);let a=artifact(&p,"sprite","image").unwrap();assert_eq!(a.sha256,c["sha256"].as_str().unwrap());}
}
