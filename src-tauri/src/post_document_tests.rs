//! Isolated regression only. Never spawns a live Codex or browser.
use crate::{engine::Engine,model::*,practical::Plan,rpc::Rpc,codex_only};
use std::{fs,path::PathBuf,sync::{Arc,atomic::Ordering},time::{Instant,Duration}};
use serde_json::json;
fn root(label:&str)->PathBuf{let p=PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").expect("isolated root required")).join(format!("post-{label}-{}",id()));fs::create_dir_all(&p).unwrap();fs::write(p.join(".ai-guild-qa"),"OFFLINE / INFERENCE 0").unwrap();p}
fn wait(e:&Arc<Engine>){let until=Instant::now()+Duration::from_secs(15);while e.worker.load(Ordering::SeqCst)&&Instant::now()<until{std::thread::sleep(Duration::from_millis(20))}assert!(!e.worker.load(Ordering::SeqCst));}
fn plan(title:&str)->Plan{Plan{title:title.into(),requirements:vec![title.into()],checklist:vec!["共有事項を確認する".into()],production_constraints:vec!["制作条件：Markdown本文だけで作成する".into()]}}
fn run(e:&Arc<Engine>,p:&std::path::Path,title:&str,valid:bool)->String{
 fs::write(p.join("offline-plan.json"),serde_json::to_vec(&plan(title)).unwrap()).unwrap();
 fs::write(p.join("offline-event.html"),if valid{format!("# {title}\n\nこのメモは、担当者が共有事項を確かめるために使います。\n\n- [ ] 共有事項を確認する\n")}else{"制作条件：Markdown本文だけで作成する".into()}).unwrap();
 *e.rpc.lock().unwrap()=Some(Rpc::phase9_fixture(p).unwrap());
 let id=e.start(format!("チーム向けに「{title}」を含むメモをMarkdownで作成してください。チェックボックスを使う。画像なし、外部調査なし。")).unwrap();wait(e);e.get_rpc().unwrap().shutdown();id
}
#[test]fn post_document_output_policy_separates_constraints_without_stripping_introductions(){
 let p=plan("朝の確認");let input="「朝の確認」をMarkdownで作成する。チェックボックス。";
 let good="# 朝の確認\n\nこのチェックリストは、公開前の見落としを減らすために使います。\n\n- [ ] 共有事項を確認する\n";
 assert!(codex_only::inspect_versioned(good,&p,input,4).is_ok());
 for prefix in ["制作条件：Markdown本文だけで作成する","構成・形式：7区分のチェックリストとして作成する。","最終出力の受入条件：Markdown本文のみ。","ご依頼どおり、7区分で作成しました。","以下の条件で作成しました。"]{assert!(codex_only::inspect_versioned(&format!("{prefix}\n{good}"),&p,input,4).is_err());assert!(codex_only::inspect_versioned(&format!("{prefix}\n{good}"),&p,input,3).is_ok());}
 assert!(codex_only::inspect_versioned(&good.replace("共有事項を確認する","省略"),&p,input,4).is_err());
 let explicit=format!("{input}「制作条件：Markdown本文だけで作成する」を表示する。");let mut pp=p.clone();pp.requirements.push(pp.production_constraints[0].clone());assert!(codex_only::inspect_versioned(&format!("{}\n{good}",pp.production_constraints[0]),&pp,&explicit,4).is_ok());
 let w=codex_only::workflow(input);assert!(w.nodes[0].work_instruction.contains("productionConstraints"));assert!(w.nodes[1].work_instruction.contains("正当な導入文"));
 let old:Plan=serde_json::from_str("{\"title\":\"旧計画\",\"requirements\":[\"旧本文\"],\"checklist\":[]}").unwrap();assert_eq!(serde_json::to_string(&old).unwrap(),"{\"title\":\"旧計画\",\"requirements\":[\"旧本文\"],\"checklist\":[]}");
 let p=root("policy");fs::write(p.join("result.json"),serde_json::to_vec_pretty(&json!({"classification":"OFFLINE_ONLY","result":"PASS","legacyVersion3Retained":true,"legitimateIntroductionRetained":true,"noAutomaticStripping":true,"explicitCopyRetained":true,"inference":0})).unwrap()).unwrap();
}
#[test]fn post_multi_job_state_logs_artifacts_restart_and_blocked_intake_are_isolated(){
 let p=root("multi");let e=Engine::open(&p,None).unwrap();let a=run(&e,&p,"仕事A",true);let sa=e.snapshot();let ja=sa.job.as_ref().unwrap();assert_eq!(ja.status,"completed");let frozen=serde_json::to_value(ja).unwrap();let product_a=ja.artifacts.iter().find(|x|x.node_id=="writing").unwrap().clone();let bytes_a=fs::read(&product_a.path).unwrap();
 let b=run(&e,&p,"仕事B",false);let sb=e.snapshot();assert_eq!(sb.job.as_ref().unwrap().status,"failed");let failed=serde_json::to_value(sb.job.as_ref().unwrap()).unwrap();
 // A mandatory ChatGPT request is rejected without a new job, peer call or change.
 let before=serde_json::to_value(&sb).unwrap();assert!(e.start("通常ChatGPTへ委譲して画像を生成する".into()).is_err());assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);assert!(!e.worker.load(Ordering::SeqCst));
 let c=run(&e,&p,"仕事C",true);let sc=e.snapshot();assert_eq!(sc.job.as_ref().unwrap().status,"completed");assert_eq!(sc.history.len(),2);assert_eq!(serde_json::to_value(&sc.history[0]).unwrap(),frozen);assert_eq!(serde_json::to_value(&sc.history[1]).unwrap(),failed);assert_eq!(fs::read(&product_a.path).unwrap(),bytes_a);crate::artifact_open::prepare(&sc,&product_a.path).unwrap();
 let mut wrong=sc.clone();wrong.job.as_mut().unwrap().artifacts.push(product_a.clone());wrong.history.clear();assert!(crate::artifact_open::prepare(&wrong,&product_a.path).is_err());
 for jid in [&a,&b,&c]{let rows=sc.events.iter().filter(|x|&x.job_id==jid).collect::<Vec<_>>();assert!(rows.iter().any(|x|x.event_type=="job_started"));assert!(sc.instructions.iter().any(|x|&x.job_id==jid));}
 let peer=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap();assert_eq!(peer.lines().count(),6);assert!(peer.lines().all(|x|serde_json::from_str::<serde_json::Value>(x).unwrap()["inference"]==false));
 let expected_jobs=serde_json::to_value((&sc.job,&sc.history)).unwrap();let expected_events=serde_json::to_value(&sc.events).unwrap();drop(e);let restored=Engine::open(&p,None).unwrap();let sr=restored.snapshot();assert_eq!(serde_json::to_value((&sr.job,&sr.history)).unwrap(),expected_jobs);assert_eq!(serde_json::to_value(&sr.events).unwrap(),expected_events);crate::artifact_open::prepare(&sr,&product_a.path).unwrap();assert!(!restored.worker.load(Ordering::SeqCst));
 let db=crate::storage::Store::open(&p.join("guild.sqlite3")).unwrap();let counts:Vec<(String,u32)>=db.0.prepare("SELECT job_id,count(*) FROM events GROUP BY job_id").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().map(Result::unwrap).collect();assert_eq!(counts.len(),3);
 fs::write(p.join("result.json"),serde_json::to_vec_pretty(&json!({"classification":"OFFLINE_ONLY","result":"PASS","jobs":[a,b,c],"statusIsolation":"PASS","logsByJob":counts,"artifactOwnership":"PASS","oneJobFailureDoesNotStopNext":"PASS","mandatoryChatgptRejectedWithoutMutation":"PASS","restartNoReplay":"PASS","completedAUnchanged":"PASS","liveInference":0,"fixtureTurnStartMessages":6,"parallelScheduler":"NOT_ADDED"})).unwrap()).unwrap();
}
#[test]fn post_concurrent_start_accepts_only_one_isolated_demo_job(){
 let p=root("duplicate");let e=Engine::open(&p,None).unwrap();e.update(|s|{s.mode="demo".into();s.workflow.nodes.truncate(1);s.workflow.nodes[0].dependencies.clear();s.workflow.edges.clear();Ok(())}).unwrap();
 let barrier=Arc::new(std::sync::Barrier::new(6));let calls=(0..6).map(|_|{let e=e.clone();let b=barrier.clone();std::thread::spawn(move||{b.wait();e.start("日本語の開始操作をfixtureで確認".into())})}).collect::<Vec<_>>();let outcomes=calls.into_iter().map(|t|t.join().unwrap()).collect::<Vec<_>>();assert_eq!(outcomes.iter().filter(|x|x.is_ok()).count(),1);wait(&e);assert_eq!(e.snapshot().events.iter().filter(|x|x.event_type=="job_started").count(),1);assert!(e.snapshot().history.is_empty());
 fs::write(p.join("result.json"),serde_json::to_vec_pretty(&json!({"result":"PASS","classification":"ISOLATED_DEMO","acceptedStarts":1,"rejectedStarts":5,"inference":0})).unwrap()).unwrap();
}

