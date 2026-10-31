//! Productization acceptance: isolated profiles + deterministic local peer only.
use crate::{engine::{Engine,verify_artifact},general,model::*,rpc::Rpc,setup::{self,Settings}};
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf},sync::{Arc,atomic::Ordering},time::{Instant,Duration}};
fn root(label:&str)->PathBuf{let p=PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").expect("explicit isolated test root")).join(format!("product-{label}-{}",id()));fs::create_dir_all(&p).unwrap();fs::write(p.join(".ai-guild-qa"),"OFFLINE ONLY; inference 0").unwrap();p}
fn settings(p:&Path)->Settings{fs::create_dir_all(p.join("output")).unwrap();Settings{work_root:p.to_string_lossy().into(),output_root:p.join("output").to_string_lossy().into(),approval_policy:"on-request".into(),..Default::default()}}
fn wait(e:&Engine){let until=Instant::now()+Duration::from_secs(15);while e.worker.load(Ordering::SeqCst)&&Instant::now()<until{std::thread::sleep(Duration::from_millis(20));}assert!(!e.worker.load(Ordering::SeqCst),"offline peer timed out");}
fn plan(files:Value)->Value{json!({"title":"納品","requirements":["指定データを納品する"],"checklist":["形式と内容を確認する"],"productionConstraints":["実行テストを装わない"],"files":files})}
fn seed(p:&Path,spec:Value,files:Value){fs::write(p.join("offline-plan.json"),serde_json::to_vec(&plan(spec)).unwrap()).unwrap();fs::write(p.join("offline-event.html"),serde_json::to_vec(&json!({"files":files})).unwrap()).unwrap();}
fn run(e:&Arc<Engine>,p:&Path,input:&str)->String{*e.rpc.lock().unwrap()=Some(Rpc::phase9_fixture(p).unwrap());let job=e.start(input.into()).unwrap();wait(e);e.get_rpc().unwrap().shutdown();job}
fn record(p:&Path,value:Value){fs::write(p.join("result.json"),serde_json::to_vec_pretty(&value).unwrap()).unwrap();}
#[test]fn product_workspace_refuses_existing_job_and_reparse_output(){
 let p=root("workspace");let fresh=crate::engine::job_workspace(&p,"job-one").unwrap();assert!(fresh.is_dir());assert!(crate::engine::job_workspace(&p,"job-one").is_err());assert!(crate::engine::job_workspace(&p,"../outside").is_err());
 #[cfg(windows)]{let linked=root("linked-output");let target=root("junction-target");assert!(std::process::Command::new("cmd.exe").args(["/c","mklink","/J"]).arg(linked.join("quests")).arg(&target).status().unwrap().success());assert!(crate::engine::job_workspace(&linked,"never-created").is_err());assert!(!target.join("never-created").exists());}
 record(&p,json!({"result":"PASS","existingJob":"NO_OVERWRITE","reparseOutput":"REJECTED_BEFORE_JOB_CREATION","liveInference":0}));
}
#[test]fn product_unauthenticated_api_key_and_protocol_errors_stop_without_turn_start(){
 let p=root("auth");
 for account in [json!({"account":null}),json!({"account":{"type":"apiKey"}})]{fs::write(p.join("offline-account.json"),account.to_string()).unwrap();assert!(Rpc::authenticated_fixture(&p).is_err());}
 fs::write(p.join("offline-account.json"),json!({"account":{"type":"chatgpt"}}).to_string()).unwrap();let(r,_)=Rpc::authenticated_fixture(&p).unwrap();r.shutdown();fs::write(p.join("offline-protocol-error"),"OFFLINE").unwrap();assert!(Rpc::authenticated_fixture(&p).is_err());assert!(!p.join("offline-peer.jsonl").exists());
 record(&p,json!({"result":"PASS","classification":"OFFLINE_AUTH_HANDSHAKE","unauthenticated":"REJECTED","apiKey":"REJECTED","protocolMismatch":"REJECTED","chatgpt":"ACCEPTED_FIXTURE_ONLY","turnStarts":0,"liveInference":0}));
}
#[test]fn product_selected_code_test_is_digest_bound_and_does_not_start_another_turn(){
 for (mode,file) in [("python-unittest","test_total.py"),("node-test","total.test.mjs")]{
  let p=root(mode);let e=Engine::open(&p,None).unwrap();e.save_setup(settings(&p)).unwrap();
  seed(&p,json!([{"name":file,"requiredText":["OFFLINE"]}]),json!([{"name":file,"content":if mode=="python-unittest"{"import unittest\n# OFFLINE fixture only\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        self.assertEqual(2 + 2, 4)\n"}else{"import test from 'node:test';\nimport assert from 'node:assert/strict';\ntest('OFFLINE',()=>assert.equal(2+2,4));\n"}}]));
  let mut plan:Value=serde_json::from_slice(&fs::read(p.join("offline-plan.json")).unwrap()).unwrap();plan["testCommand"]=json!(mode);fs::write(p.join("offline-plan.json"),plan.to_string()).unwrap();fs::write(p.join("offline-code-test.json"),json!({"exitCode":0,"stdout":"OFFLINE TEST FIXTURE PASS","stderr":""}).to_string()).unwrap();
  run(&e,&p,"コードを制作してテストする");let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"completed");general::verify_product(j).unwrap();let report=j.artifacts.iter().find(|a|a.kind=="test").unwrap();let r:Value=serde_json::from_slice(&fs::read(&report.path).unwrap()).unwrap();assert_eq!(r["response"]["exitCode"],0);assert_eq!(r["testCommand"],mode);
  let calls=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap();assert_eq!(calls.lines().filter(|line|line.contains("\"event\": \"turn/start\"")).count(),2);assert_eq!(calls.lines().filter(|line|line.contains("\"method\": \"command/exec\"")).count(),1);
  let f=j.artifacts.iter().find(|a|a.kind=="product").unwrap();fs::write(&f.path,"changed code").unwrap();assert!(general::verify_product(j).is_err());
  record(&p,json!({"result":"PASS","classification":"OFFLINE_CODE_EXECUTION_FIXTURE","mode":mode,"turnStarts":2,"selectedTestCalls":1,"digestTamper":"REJECTED","liveCodeExecution":"NOT_VERIFIED","liveInference":0}));
 }
}
#[test]fn product_code_test_failed_or_unknown_keeps_fault_and_prevents_replay(){
 for result in [json!({"exitCode":1,"stdout":"","stderr":"OFFLINE failure"}),json!({"error":{"code":-32000,"message":"OFFLINE unknown command outcome"}})]{
  let p=root("code-ng");let e=Engine::open(&p,None).unwrap();e.save_setup(settings(&p)).unwrap();
  seed(&p,json!([{"name":"test_total.py","requiredText":["OFFLINE"]}]),json!([{"name":"test_total.py","content":"# OFFLINE\n"}]));let mut plan:Value=serde_json::from_slice(&fs::read(p.join("offline-plan.json")).unwrap()).unwrap();plan["testCommand"]=json!("python-unittest");fs::write(p.join("offline-plan.json"),plan.to_string()).unwrap();fs::write(p.join("offline-code-test.json"),result.to_string()).unwrap();run(&e,&p,"Pythonで制作してテストする");let s=e.snapshot();assert_eq!(s.job.as_ref().unwrap().status,"failed");assert!(s.job.as_ref().unwrap().quality.is_none());assert!(s.events.iter().any(|e|e.event_type=="execution_error"));let j=s.job.as_ref().unwrap();let n=j.workflow.nodes.iter().find(|n|n.node_id=="qa").unwrap();assert!(general::run_tests(&e,j,n).is_err());let calls=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap();assert_eq!(calls.lines().filter(|line|line.contains("\"method\": \"command/exec\"")).count(),1);
  record(&p,json!({"result":"PASS","codeFailureStopsDelivery":true,"replayedCommands":0,"faultHistoryRetained":true,"liveInference":0}));
 }
}

#[test]fn product_clean_profile_setup_isolation_missing_codex_and_versions(){
 let a=root("clean-a");let b=root("clean-b");let e=Engine::open(&a,None).unwrap();let s=e.snapshot();assert!(!s.settings.setup_complete);assert!(s.history.is_empty()&&s.job.is_none());assert_eq!(s.connection.status,"disconnected");assert!(s.workflow.nodes.iter().all(|n|n.executor!="manual"));
 assert!(Engine::open(&a,None).is_err());let other=Engine::open(&b,None).unwrap();assert_ne!(other.snapshot().data_path,s.data_path);
 let before=serde_json::to_value(&s).unwrap();assert!(e.start("文書を作成".into()).is_err());assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);
 let absent=setup::inspect(a.join("missing/codex.exe").to_str().unwrap());assert_eq!(absent["available"],false);assert_eq!(absent["inferencePerformed"],false);
 for v in ["codex-cli 0.114.0","codex-cli 0.153.4","codex 1.0.0"]{setup::check_version(v).unwrap();}for v in ["codex-cli 0.100.0","unrecognised"]{assert!(setup::check_version(v).is_err());}
 let mut invalid=settings(&a);invalid.approval_policy="never".into();assert!(e.save_setup(invalid).is_err());e.save_setup(settings(&a)).unwrap();assert!(e.snapshot().settings.setup_complete);drop(e);let restored=Engine::open(&a,None).unwrap();assert!(restored.snapshot().settings.setup_complete);assert_eq!(restored.snapshot().settings.output_root,settings(&a).output_root);
 let serialized=serde_json::to_string(&restored.snapshot().settings).unwrap();assert!(!serialized.contains("token")&&!serialized.contains("password"));
 record(&a,json!({"result":"PASS","cleanProfile":true,"missingCodex":"SAFE_STOP","unsupportedVersion":"SAFE_STOP","exclusiveProfile":"PASS","settingsRestart":"PASS","otherProfileEmpty":other.snapshot().job.is_none(),"liveInference":0}));
}
#[test]fn product_generic_csv_json_code_multifile_restart_and_history(){
 let p=root("generic");let e=Engine::open(&p,None).unwrap();e.save_setup(settings(&p)).unwrap();
 seed(&p,json!([{"name":"items.csv","requiredText":["りんご"]},{"name":"items.json","requiredText":["りんご"]},{"name":"total.py","requiredText":["def total"]},{"name":"README.md","requiredText":["実行テストは未実施"]}]),json!([{"name":"items.csv","content":"品名,数量\nりんご,3\n"},{"name":"items.json","content":"{\"品名\":\"りんご\",\"数量\":3}"},{"name":"total.py","content":"def total(items):\n    return sum(items)\n"},{"name":"README.md","content":"# 利用手順\n実行テストは未実施\n"}]));
 let a=run(&e,&p,"ローカルのCSV・JSON・Pythonコードと利用手順を制作する。");let sa=e.snapshot();let ja=sa.job.as_ref().unwrap();assert_eq!(ja.status,"completed");assert_eq!(ja.workflow_id,general::ID);assert_eq!(ja.artifacts.iter().filter(|a|a.kind=="product").count(),4);assert!(Path::new(&ja.workspace).canonicalize().unwrap().starts_with(p.join("output").canonicalize().unwrap()));general::verify_product(ja).unwrap();
 for f in ja.artifacts.iter().filter(|a|a.kind=="product"){verify_artifact(f,Path::new(&ja.workspace)).unwrap();crate::artifact_open::prepare(&sa,&f.path).unwrap();}
 let frozen=serde_json::to_value(ja).unwrap();seed(&p,json!([{"name":"notes.md","requiredText":["引継ぎ"]}]),json!([{"name":"notes.md","content":"# 引継ぎ\n担当者が共有内容を確認する。"}]));let b=run(&e,&p,"引継ぎメモを作成する");let sb=e.snapshot();assert_ne!(a,b);assert_eq!(sb.history.len(),1);assert_eq!(serde_json::to_value(&sb.history[0]).unwrap(),frozen);
 for jid in [&a,&b]{assert!(sb.events.iter().any(|r|&r.job_id==jid&&r.event_type=="job_completed"));}let frozen_jobs=serde_json::to_value((&sb.job,&sb.history)).unwrap();drop(e);let restored=Engine::open(&p,None).unwrap();let sr=restored.snapshot();assert_eq!(serde_json::to_value((&sr.job,&sr.history)).unwrap(),frozen_jobs);assert!(!restored.worker.load(Ordering::SeqCst));
 let peer=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap();assert_eq!(peer.lines().count(),4);assert!(peer.lines().all(|v|serde_json::from_str::<Value>(v).unwrap()["inference"]==false));
 record(&p,json!({"result":"PASS","classification":"OFFLINE_FIXTURE","jobs":[a,b],"multifile":4,"formats":["CSV","JSON","Python source","Markdown"],"restartNoReplay":true,"historyUnchanged":true,"liveInference":0,"runtimeCodeTests":"NOT_VERIFIED"}));
}
#[test]fn product_generic_missing_copy_invalid_csv_paths_and_tampering_stop(){
 let p=root("bad-output");let e=Engine::open(&p,None).unwrap();e.save_setup(settings(&p)).unwrap();
 seed(&p,json!([{"name":"data.csv","requiredText":["確認"]}]),json!([{"name":"data.csv","content":"項目,値\n確認,=HYPERLINK(1)\n"}]));run(&e,&p,"CSVを作成する");let s=e.snapshot();assert_eq!(s.job.as_ref().unwrap().status,"failed");assert!(s.job.as_ref().unwrap().quality.is_none());assert!(!Path::new(&s.job.as_ref().unwrap().workspace).join("data.csv").exists());
 for name in ["../x.md","C:\\x.md","CON.txt","com1.py","manifest.json","run.exe","X.MD:evil","data.csv "]{assert!(!general::safe_name(name),"{name}")}
 let spec=json!([{"name":"data.json","requiredText":["必須"]}]);let valid=plan(spec);assert!(general::parse_plan(&valid.to_string(),"「別の指定」を含む").is_err());
 seed(&p,json!([{"name":"data.json","requiredText":["確認"]}]),json!([{"name":"data.json","content":"{\"確認\":true}"}]));run(&e,&p,"JSONで作成する");let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"completed");let f=j.artifacts.iter().find(|f|f.kind=="product").unwrap();fs::write(&f.path,"{}").unwrap();assert!(general::verify_product(j).is_err());assert!(crate::artifact_open::prepare(&s,&f.path).is_err());
 let before=serde_json::to_value(e.snapshot()).unwrap();for request in ["通常ChatGPTへ画像を依頼","画像を生成して","SNSへ投稿して","既存ファイルを削除して","ブラウザで購入して"]{assert!(e.start(request.into()).is_err());}assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);
 record(&p,json!({"result":"PASS","formulaInjection":"REJECTED","unsafeNames":"REJECTED","missingIndependentCopy":"REJECTED","changedArtifact":"REJECTED","unsupportedActionsNoMutation":true,"failureHistoryRetained":s.events.iter().any(|e|e.event_type=="execution_error"),"liveInference":0}));
}
#[test]fn product_selected_input_is_copied_exactly_and_original_is_untouched(){
 let p=root("inputs");let original=p.join("original.csv");fs::write(&original,"項目,値\n確認,2\n").unwrap();let before=fs::read(&original).unwrap();let mut configuration=settings(&p);configuration.input_paths.push(original.to_string_lossy().into());let e=Engine::open(&p,None).unwrap();e.save_setup(configuration.clone()).unwrap();
 seed(&p,json!([{"name":"converted.json","requiredText":["確認"]}]),json!([{"name":"converted.json","content":"{\"確認\":2}"}]));run(&e,&p,"入力のCSVをJSONへ整理する");let s=e.snapshot();let j=s.job.as_ref().unwrap();assert_eq!(j.status,"completed");let input=j.artifacts.iter().find(|a|a.kind=="input").unwrap();assert_eq!(fs::read(&input.path).unwrap(),before);assert_eq!(fs::read(&original).unwrap(),before);assert!(Path::new(&input.path).canonicalize().unwrap().starts_with(Path::new(&j.workspace).canonicalize().unwrap()));
 let external=root("outside").join("external.csv");fs::write(&external,"a,b").unwrap();configuration.input_paths=vec![external.to_string_lossy().into()];assert!(general::inputs(&configuration).is_err());configuration.input_paths=vec![original.to_string_lossy().into()];fs::write(&original,"api_key=never-expose-this-secret").unwrap();assert!(general::inputs(&configuration).is_err());
 let secret_json=p.join("settings.json");fs::write(&secret_json,json!({"nested":{"access_token":"OFFLINE_CREDENTIAL_FIXTURE"}}).to_string()).unwrap();configuration.input_paths=vec![secret_json.to_string_lossy().into()];assert!(general::inputs(&configuration).is_err());
 record(&p,json!({"result":"PASS","sourceCopiedExact":true,"sourceUnchangedDuringJob":true,"outsideInput":"REJECTED","secretInput":"REJECTED","jsonCredential":"REJECTED","liveInference":0}));
}
#[test]fn product_generic_formal_response_recovery_preserves_original_attempt(){
 use crate::recovery::{self,Contract,Binding,sha};
 let p=root("recovery");let e=Engine::open(&p,None).unwrap();e.save_setup(settings(&p)).unwrap();seed(&p,json!([{"name":"notes.md","requiredText":["確認"]}]),json!([{"name":"notes.md","content":"# 確認\n復旧用OFFLINE fixture"}]));let bundle=fs::read_to_string(p.join("offline-event.html")).unwrap();fs::write(p.join("offline-event.html"),"{}").unwrap();run(&e,&p,"メモを作成する");assert_eq!(e.snapshot().job.as_ref().unwrap().status,"failed");
 let mut s=e.snapshot();let j=s.job.as_mut().unwrap();let plan_raw=fs::read_to_string(j.artifacts.iter().find(|a|a.name=="plan.json").unwrap().path.clone()).unwrap();
 let failed_run=j.nodes["writing"].clone();j.artifacts.retain(|a|a.node_id=="plan");j.quality=None;j.status="failed".into();j.active_node_id=Some("writing".into());j.nodes.get_mut("writing").unwrap().status="failed".into();for node in ["qa","delivery"]{j.nodes.insert(node.into(),Run::default());}
 // Dedicated offline crash fixture only; never edits an actual completed Job.
 // Keep the original mock thread/turn identities: S2 bindings are immutable.
 s.history.clear();s.event(Some("writing"),"fixture_crash","failed","OFFLINE injected after formal completion",None,vec![]);e.data.lock().unwrap().snapshot=s.clone();
 let j=s.job.as_ref().unwrap();let mut bindings=vec![];let mut items=serde_json::Map::new();let mut turns=vec![];
 for(node,raw)in [("plan",plan_raw),("writing",bundle)]{let turn=s.task_executions.iter().find(|b|b.job_id==j.job_id&&b.node_id==node&&b.attempt==j.nodes[node].attempt).unwrap().turn_id.clone().unwrap();let prompt=recovery::prompt(&s,j,node).unwrap();bindings.push(Binding{node_id:node.into(),turn_id:turn.clone(),response_sha256:sha(raw.as_bytes()),response_bytes:raw.len() as u64,request_sha256:sha(prompt.as_bytes())});turns.push(json!({"id":turn,"status":"completed"}));items.insert(turn.clone(),json!({"data":[{"turnId":turn,"item":{"type":"userMessage","content":[{"type":"text","text":prompt}]}},{"turnId":turn,"item":{"type":"agentMessage","phase":"final_answer","text":raw}}],"nextCursor":null}));}
 let c=Contract{job_id:j.job_id.clone(),thread_id:j.thread_id.clone().unwrap(),turns:bindings};fs::write(p.join("proof.json"),serde_json::to_vec(&c).unwrap()).unwrap();fs::write(p.join("offline-recovery-capture.json"),serde_json::to_vec(&json!({"thread":{"thread":{"id":c.thread_id,"cwd":j.workspace}},"turns":{"data":turns,"nextCursor":null},"items":items})).unwrap()).unwrap();fs::write(p.join("offline-peer.jsonl"),"").unwrap();
 *e.rpc.lock().unwrap()=Some(Rpc::phase9_fixture(&p).unwrap());e.recover_formal(p.join("proof.json").to_string_lossy().into()).unwrap();let result=e.snapshot();let j=result.job.as_ref().unwrap();assert_eq!(j.status,"completed");assert_eq!(j.nodes["writing"].attempt,failed_run.attempt);general::verify_product(j).unwrap();assert_eq!(result.task_executions.len(),4);for original in &s.task_executions{assert!(result.task_executions.contains(original));}assert!(result.events.iter().any(|e|e.event_type=="fixture_crash"));let calls=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap();assert!(!calls.contains("turn/start"));
 record(&p,json!({"result":"PASS","sameJob":c.job_id,"sameAttempt":failed_run.attempt,"formalResultRecovery":true,"newTurnStarts":0,"faultHistoryPreserved":true,"liveInference":0}));e.get_rpc().unwrap().shutdown();
}
