use serde_json::{json,Value};
use std::{collections::HashMap,io::{BufRead,BufReader,Write},process::{Child,ChildStdin,Command,Stdio},sync::{Arc,Mutex,mpsc,atomic::{AtomicBool,AtomicU64,Ordering}},time::Duration};
#[cfg(windows)] use std::os::windows::process::CommandExt;
type Response=mpsc::Sender<Result<Value,String>>;
pub struct Rpc {pub writer:Mutex<ChildStdin>,pub child:Mutex<Child>,pub alive:AtomicBool,serial:AtomicU64,pending:Mutex<HashMap<u64,Response>>,pub events:Mutex<Option<mpsc::Sender<Value>>>,readonly_verified:AtomicBool,#[cfg(any(test,feature="qa-harness"))]mock_peer:AtomicBool,#[cfg(feature="qa-harness")]live:Mutex<Option<crate::qa_live::LiveTrial>>}
pub const READONLY_OVERRIDES:[&str;8]=["-c","sandbox_mode=\"read-only\"","-c","approval_policy=\"on-request\"","-c","windows.sandbox=\"unelevated\"","-c","web_search=\"disabled\""];
pub fn readonly_policy()->Value{json!({"type":"readOnly","networkAccess":false})}
fn validate_readonly_config(v:&Value)->Result<(),String>{
 if v["config"]["sandbox_mode"]!="read-only"||v["config"]["approval_policy"]!="on-request"||v["config"]["windows"]["sandbox"]!="unelevated"{return Err("実効sandbox/承認設定が標準read-only/on-request/unelevatedと一致しません。実行を停止します。".into())}Ok(())
}
pub fn validate_readonly_thread(v:&Value)->Result<(),String>{
 if v["sandbox"]!=readonly_policy()||v["approvalPolicy"]!="on-request"{return Err("Codexが確認したthread権限がread-only/on-requestではありません。turnを開始しません。".into())}Ok(())
}
pub fn command(path:impl AsRef<std::ffi::OsStr>)->Command {let mut c=Command::new(path);#[cfg(windows)]c.creation_flags(0x08000000);c}
pub fn codex_path()->Result<String,String>{
 if let Ok(o)=command("where.exe").arg("codex.exe").output(){if o.status.success(){if let Some(p)=String::from_utf8_lossy(&o.stdout).lines().next(){return Ok(p.trim().into())}}}
 let base=std::env::var("LOCALAPPDATA").map_err(|_|"Codexが見つかりません。".to_string())?;let path=std::path::Path::new(&base).join("OpenAI/Codex/bin");
 let mut found=std::fs::read_dir(path).map_err(|_|"Codex実行ファイルが見つかりません。".to_string())?.flatten().map(|e|e.path().join("codex.exe")).filter(|p|p.is_file()).collect::<Vec<_>>();found.sort();found.pop().map(|p|p.to_string_lossy().into()).ok_or("Codexが見つかりません。".into())
}
impl Rpc{
 #[cfg(test)]pub fn authenticated_fixture(root:&std::path::Path)->Result<(Arc<Self>,String),String>{Self::initialize(Self::phase9_fixture(root)?,"OFFLINE auth handshake".into())}
 #[cfg(test)]
 pub fn phase9_fixture(root:&std::path::Path)->Result<Arc<Self>,String>{
  if !root.join(".ai-guild-qa").is_file(){return Err("Isolated offline marker required".into())}
  let python=std::env::var_os("AI_GUILD_QA_PEER_PYTHON").ok_or("Explicit test Python required")?;
  let script=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("scripts/phase9-offline-peer.py");
  let child=command(python).args(["-X","utf8"]).arg(script).arg(root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;
  Self::attach(child)
 }
 #[cfg(feature="qa-harness")]pub fn spawn()->Result<(Arc<Self>,String),String>{
  Self::spawn_for("")
 }
 pub fn spawn_for(configured:&str)->Result<(Arc<Self>,String),String>{
  #[cfg(feature="qa-harness")]
  {if std::env::var_os("AI_GUILD_QA_PEER_PYTHON").is_some()&&std::env::var_os("AI_GUILD_QA_LIVE_PERMIT").is_some(){return Err("Conflicting mock/live QA transport settings".into())}if std::env::var_os("AI_GUILD_QA_PEER_PYTHON").is_some(){return Self::spawn_qa_peer()}if let Some(path)=std::env::var_os("AI_GUILD_QA_LIVE_PERMIT"){return Self::spawn_qa_live(std::path::Path::new(&path))}}
  let exe=if configured.is_empty(){codex_path()?}else{crate::setup::validate_exe(configured)?;configured.into()};let version=command(&exe).arg("--version").output().map_err(|e|e.to_string())?;
  if !version.status.success(){return Err("codex --versionに失敗しました".into())}crate::setup::check_version(&String::from_utf8_lossy(&version.stdout))?;crate::setup::check_readonly_version(&String::from_utf8_lossy(&version.stdout))?;
  let child=command(&exe).args(["app-server","--listen","stdio://","-c","forced_login_method=\"chatgpt\"","-c","model_provider=\"openai\"","-c","mcp_servers={}","-c","apps._default.enabled=false","-c","features.multi_agent=false"]).args(READONLY_OVERRIDES).env_remove("OPENAI_API_KEY").env_remove("CODEX_API_KEY").env_remove("OPENAI_BASE_URL").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e|format!("app-serverを起動できません: {e}"))?;
  let rpc=Self::attach(child)?;
  let(rpc,version)=Self::initialize(rpc,String::from_utf8_lossy(&version.stdout).trim().to_string())?;
  let checked=(||{validate_readonly_config(&rpc.call("config/read",json!({"includeLayers":false}))?)?;if rpc.call("windowsSandbox/readiness",json!({}))?["status"]!="ready"{return Err("Windows sandboxがreadyではありません。実行を停止します。".into())}Ok::<_,String>(())})();
  if let Err(e)=checked{rpc.shutdown();return Err(e)}rpc.readonly_verified.store(true,Ordering::SeqCst);Ok((rpc,version))
 }
 fn attach(mut child:Child)->Result<Arc<Self>,String>{
  let writer=child.stdin.take().ok_or("stdinを取得できません")?;let stdout=child.stdout.take().ok_or("stdoutを取得できません")?;
  let rpc=Arc::new(Self{writer:Mutex::new(writer),child:Mutex::new(child),alive:AtomicBool::new(true),serial:AtomicU64::new(1),pending:Mutex::new(HashMap::new()),events:Mutex::new(None),readonly_verified:AtomicBool::new(false),#[cfg(any(test,feature="qa-harness"))]mock_peer:AtomicBool::new(false),#[cfg(feature="qa-harness")]live:Mutex::new(None)});
  let weak=Arc::downgrade(&rpc);
  std::thread::spawn(move||{for line in BufReader::new(stdout).lines(){let Some(r)=weak.upgrade()else{break};let Ok(line)=line else{break};if line.len()>16*1024*1024{continue}let Ok(v)=serde_json::from_str::<Value>(&line)else{continue};
   #[cfg(feature="qa-harness")]if let Some(live)=r.live.lock().unwrap().as_ref(){let _=live.trace("RECEIVED",&v);}
   if v.get("method").is_none(){if let Some(id)=v["id"].as_u64(){if let Some(tx)=r.pending.lock().unwrap().remove(&id){let result=if v.get("error").is_some(){Err(v["error"]["message"].as_str().unwrap_or("app-serverエラー").to_string())}else{Ok(v["result"].clone())};let _=tx.send(result);}}}
   else if let Some(tx)=r.events.lock().unwrap().as_ref(){let _=tx.send(v);}else if v.get("id").is_some(){let _=r.send(json!({"id":v["id"],"error":{"code":-32601,"message":"No active AI GUILD job accepts this request"}}));}
  }
  if let Some(r)=weak.upgrade(){r.alive.store(false,Ordering::SeqCst);for(_,tx)in r.pending.lock().unwrap().drain(){let _=tx.send(Err("app-serverの接続が切れました。実行結果は未確認です。".into()));}if let Some(tx)=r.events.lock().unwrap().as_ref(){let _=tx.send(json!({"method":"guild/disconnected"}));}}
  });
  Ok(rpc)
 }
 fn initialize(rpc:Arc<Self>,version:String)->Result<(Arc<Self>,String),String>{
  let result=rpc.call("initialize",json!({"clientInfo":{"name":"ai_guild","title":"AI GUILD","version":"0.1.0"}}));
  if let Err(e)=result{rpc.shutdown();return Err(e)}rpc.send(json!({"method":"initialized","params":{}}))?;
  let auth=rpc.call("account/read",json!({"refreshToken":false}))?;
  if auth["account"]["type"]!="chatgpt"{rpc.shutdown();return Err("ChatGPTアカウント認証が必要です。APIキー認証は利用しません。Codexでログインを確認してください。".into())}
  Ok((rpc,version))
 }
 #[cfg(any(test,feature="qa-harness"))]
 pub fn spawn_qa_peer()->Result<(Arc<Self>,String),String>{
  let qa=std::path::PathBuf::from(std::env::var_os("AI_GUILD_QA_DATA_DIR").ok_or("QA peer requires explicit QA directory")?).canonicalize().map_err(|e|e.to_string())?;
  if !qa.join(".ai-guild-qa").is_file(){return Err("QA peer requires isolation marker".into())}
  let python=std::env::var_os("AI_GUILD_QA_PEER_PYTHON").ok_or("QA peer requires explicit local Python")?;
  let script=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("tests/helpers/mock_peer.py");
  let child=command(python).args(["-X","utf8","-u"]).arg(script).arg(&qa).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;
  let rpc=Self::attach(child)?;rpc.mock_peer.store(true,Ordering::SeqCst);
  Self::initialize(rpc,"MOCK / Phase7 isolated Python peer / NO INFERENCE".into())
 }
 // Secret-capable real tool emission and persistence have not been verified.
 // Only an actually attached, compile-gated offline peer may exercise the UI.
 pub fn validate_question_support(&self,params:&Value)->Result<(),String>{
  #[cfg(any(test,feature="qa-harness"))]
  if self.mock_peer.load(Ordering::SeqCst){return Ok(())}
  validate_real_questions(params)
 }
 pub fn send(&self,v:Value)->Result<(),String>{#[cfg(feature="qa-harness")]if let Some(live)=self.live.lock().unwrap().as_ref(){live.trace("WRITE_INTENT",&v)?;}let mut w=self.writer.lock().map_err(|_|"接続ロックエラー")?;let line=serde_json::to_string(&v).map_err(|e|e.to_string())?;w.write_all(line.as_bytes()).and_then(|_|w.write_all(b"\n")).and_then(|_|w.flush()).map_err(|e|format!("stdio送信エラー: {e}"))}
 pub fn call(&self,method:&str,params:Value)->Result<Value,String>{
  if !self.alive.load(Ordering::SeqCst){return Err("app-serverは未接続です。".into())}
  if self.readonly_verified.load(Ordering::SeqCst){
   if method=="turn/start"&&(params["sandboxPolicy"]!=readonly_policy()||params["approvalPolicy"]!="on-request"){return Err("標準read-only/on-request以外のturnは送信しません。".into())}
   if method=="thread/start"&&(params["sandbox"]!="read-only"||params["approvalPolicy"]!="on-request"){return Err("標準read-only/on-request以外のthreadは送信しません。".into())}
  }
  let id=self.serial.fetch_add(1,Ordering::SeqCst);
  #[cfg(feature="qa-harness")]let params={let mut params=params;if let Some(live)=self.live.lock().unwrap().as_ref(){live.prepare(method,&mut params,id)?;}params};
  let(tx,rx)=mpsc::channel();self.pending.lock().unwrap().insert(id,tx);
  if let Err(e)=self.send(json!({"id":id,"method":method,"params":params})){self.pending.lock().unwrap().remove(&id);return Err(e)}
  let seconds=45;#[cfg(feature="qa-harness")]let seconds=if method=="initialize"&&self.live.lock().unwrap().is_some(){180}else{seconds};
  match rx.recv_timeout(Duration::from_secs(seconds)){Ok(r)=>{let v=r?;if method=="thread/start"&&self.readonly_verified.load(Ordering::SeqCst){validate_readonly_thread(&v)?;}Ok(v)},Err(_)=>{self.pending.lock().unwrap().remove(&id);Err(format!("{method}: 応答待ち上限。結果は不明です。再送せず接続・元スレッドを確認してください。"))}}
 }
 #[cfg(feature="qa-harness")]
 fn spawn_qa_live(permit:&std::path::Path)->Result<(Arc<Self>,String),String>{
  let live=crate::qa_live::LiveTrial::load(permit)?;let exe=codex_path()?;let version=command(&exe).arg("--version").output().map_err(|e|e.to_string())?;
  let mut process=command(&exe);process.args(["app-server","--listen","stdio://","-c","forced_login_method=\"chatgpt\"","-c","model_provider=\"openai\"","-c","mcp_servers={}","-c","apps._default.enabled=false","-c","features.multi_agent=false"]);
  process.args(["-c","features.multi_agent_v2=false","-c","features.goals=false"]);
  process.arg("-c").arg(format!("sqlite_home={}",serde_json::to_string(&live.sqlite.to_string_lossy().trim_start_matches("\\\\?\\").replace('\\',"/")).unwrap()));process.arg("-c").arg(format!("log_dir={}",serde_json::to_string(&live.logs.to_string_lossy().trim_start_matches("\\\\?\\").replace('\\',"/")).unwrap()));
  let child=process.env("RUST_LOG","off").env_remove("OPENAI_API_KEY").env_remove("CODEX_API_KEY").env_remove("OPENAI_BASE_URL").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;
  let maximum=live.maxima()?;let rpc=Self::attach(child)?;*rpc.live.lock().unwrap()=Some(live);let(rpc,version)=Self::initialize(rpc,format!("QA REAL / {} / ephemeral / max {} jobs {} turns",String::from_utf8_lossy(&version.stdout).trim(),maximum.0,maximum.1))?;
  let actual=rpc.call("config/read",json!({"includeLayers":false}))?;
  {let held=rpc.live.lock().unwrap();let live=held.as_ref().unwrap();for(key,expected)in [("sqlite_home",&live.sqlite),("log_dir",&live.logs)]{let p=actual["config"][key].as_str().ok_or("Server did not confirm isolation configuration")?;if std::path::Path::new(p).canonicalize().map_err(|e|e.to_string())?!=*expected{rpc.shutdown();return Err("Server isolation differs from approved proof".into())}}if actual["config"]["model"].as_str()!=Some(&live.model){rpc.shutdown();return Err("Current model differs from verified existing model".into())}fs_write_isolation(&live.root,&actual,&live.permit)?;}
  Ok((rpc,version))
 }
 pub fn shutdown(&self){self.alive.store(false,Ordering::SeqCst);if let Ok(mut c)=self.child.lock(){let _=c.kill();let _=c.wait();}}
}
impl Drop for Rpc{fn drop(&mut self){self.shutdown();}}
fn validate_real_questions(params:&Value)->Result<(),String>{
 if params["questions"].as_array().is_some_and(|qs|qs.iter().any(|q|q["isSecret"]==true)){
  return Err("秘密指定質問は実サーバーでの安全性が未検証のため、この版では回答を送信できません。通常の質問欄にも秘密を入力しないでください。".into())
 }
 Ok(())
}
#[cfg(test)]mod question_support_tests{
 use super::*;
 #[test]fn readonly_contract_rejects_unknown_versions_and_effective_escalation(){
  assert!(crate::setup::check_readonly_version("codex-cli 0.164.0-alpha.11.1\r\n").is_ok());
  assert!(crate::setup::check_readonly_version("codex-cli 0.166.0-alpha.5.1\r\n").is_ok());
  for version in ["codex-cli 0.114.0","codex-cli 0.164.0-alpha.11.2","codex-cli 0.166.0-alpha.5.2","codex-cli 0.166.0","codex-cli 0.167.0-alpha.1","codex-cli 1.0.0","UNKNOWN"]{assert!(crate::setup::check_readonly_version(version).is_err());}
  let good=json!({"sandbox":readonly_policy(),"approvalPolicy":"on-request"});assert!(validate_readonly_thread(&good).is_ok());
  for bad in [json!({}),json!({"sandbox":{"type":"dangerFullAccess"},"approvalPolicy":"on-request"}),json!({"sandbox":{"type":"readOnly","networkAccess":true},"approvalPolicy":"on-request"}),json!({"sandbox":readonly_policy(),"approvalPolicy":"never"})]{assert!(validate_readonly_thread(&bad).is_err());}
  let good=json!({"config":{"sandbox_mode":"read-only","approval_policy":"on-request","windows":{"sandbox":"unelevated"}}});assert!(validate_readonly_config(&good).is_ok());
  for key in ["sandbox_mode","approval_policy","windows"]{let mut bad=good.clone();bad["config"][key]=Value::Null;assert!(validate_readonly_config(&bad).is_err());}
 }
 #[test]fn real_secret_questions_are_rejected_without_affecting_normal_questions(){
  assert!(validate_real_questions(&json!({"questions":[{"id":"normal","isSecret":false}]})).is_ok());
  assert!(validate_real_questions(&json!({"questions":[{"id":"normal","isSecret":false},{"id":"private","isSecret":true}]})).is_err());
 }
 #[test]fn unverified_secret_request_and_restored_answer_are_blocked_on_real_policy(){
  use crate::{engine::Engine,model::*};use std::{fs,time::Instant};
  let root=std::path::PathBuf::from(std::env::var_os("AI_GUILD_TEST_ROOT").unwrap()).join(format!("secret-policy-{}",id()));
  fs::create_dir_all(&root).unwrap();fs::write(root.join(".ai-guild-qa"),"OFFLINE secret support policy").unwrap();std::env::set_var("AI_GUILD_QA_DATA_DIR",&root);
  let e=Engine::open(&root,None).unwrap();let(rpc,_)=Rpc::spawn_qa_peer().unwrap();
  // Real policy on an offline transport: no real server or model is called.
  rpc.mock_peer.store(false,Ordering::SeqCst);*e.rpc.lock().unwrap()=Some(rpc.clone());
  e.update(|s|{s.connection.status="connected".into();s.workflow.nodes.truncate(1);s.workflow.edges.clear();s.workflow.nodes[0].work_instruction="QA_CASE=questions".into();Ok(())}).unwrap();
  e.start("OFFLINE secret policy; never ask a human for a secret".into()).unwrap();let until=Instant::now()+Duration::from_secs(8);
  while e.worker.load(Ordering::SeqCst)&&Instant::now()<until{std::thread::sleep(Duration::from_millis(20));}
  assert!(!e.worker.load(Ordering::SeqCst));let s=e.snapshot();
  assert!(s.approvals.is_empty());assert_eq!(s.events.iter().filter(|v|v.event_type=="secret_request_rejected").count(),1);
  let records=fs::read_to_string(root.join("mock-peer.jsonl")).unwrap();
  let rows=records.lines().map(|v|serde_json::from_str::<Value>(v).unwrap()).collect::<Vec<_>>();
  let response=rows.iter().find(|v|v["event"]=="response").unwrap();assert_eq!(response["questionIds"],json!([]));
  let request_id=rows.iter().find(|v|v["event"]=="request").unwrap()["requestId"].clone();
  let aid=id();{
   // A restored pending request cannot bypass the send-side guard.
   let mut d=e.data.lock().unwrap();let mut snapshot=d.snapshot.clone();let j=snapshot.job.as_mut().unwrap();j.status="awaiting_approval".into();j.active_node_id=Some("research".into());j.turn_id=Some("offline-restored-turn".into());j.nodes.get_mut("research").unwrap().status="awaiting_approval".into();
   snapshot.approvals.push(Approval{id:aid.clone(),request_id,job_id:j.job_id.clone(),node_id:"research".into(),kind:"item/tool/requestUserInput".into(),title:"OFFLINE restored secret request".into(),details:String::new(),params:json!({"threadId":j.thread_id,"turnId":j.turn_id,"questions":[{"id":"private","isSecret":true}]}),resolved:false,decision:None});
   d.store.save(&snapshot).unwrap();d.snapshot=snapshot;
  }
  let err=e.answer_structured(aid,"accept".into(),String::new(),None).unwrap_err();assert!(err.contains("未検証"));
  assert_eq!(fs::read_to_string(root.join("mock-peer.jsonl")).unwrap(),records);rpc.shutdown();
 }
}
#[cfg(feature="qa-harness")]
fn fs_write_isolation(root:&std::path::Path,actual:&Value,permit:&std::path::Path)->Result<(),String>{let budget:Value=serde_json::from_slice(&std::fs::read(permit).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;std::fs::write(root.join(format!("native-server-isolation-{}.json",crate::model::id())),serde_json::to_vec_pretty(&json!({"at":crate::model::now(),"method":"config/read","sqlite_home":actual["config"]["sqlite_home"],"log_dir":actual["config"]["log_dir"],"model":actual["config"]["model"],"turnStartsReservedAtCheck":budget["turnsReserved"]})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())}
