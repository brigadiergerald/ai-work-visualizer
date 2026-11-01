//! Compiled only into the explicitly labelled Phase 7 QA executable.
//! No fallback, credential copy, auto-approval, or production activation.
use std::{fs,path::{Path,PathBuf},io::Write};
use serde_json::{Value,json};use sha2::{Sha256,Digest};
pub struct LiveTrial {pub permit:PathBuf,pub root:PathBuf,pub sqlite:PathBuf,pub logs:PathBuf,pub model:String,authority:PathBuf,authority_sha:String,connection_id:String}
struct Lock(PathBuf);impl Drop for Lock{fn drop(&mut self){let _=fs::remove_file(&self.0);}}
fn canonical(p:&Path)->Result<PathBuf,String>{p.canonicalize().map_err(|e|e.to_string())}
fn hash(p:&Path)->Result<String,String>{Ok(crate::hashing::hex(&Sha256::digest(fs::read(p).map_err(|e|e.to_string())?)))}
fn limits(v:&Value)->Result<(u64,u64),String>{
 let mut maximum=(2,6);
 if let Some(amendment)=v.get("amendment"){
  let path=Path::new(amendment["path"].as_str().ok_or("Missing amendment source")?);if hash(path)?!=amendment["sha256"].as_str().unwrap_or(""){return Err("Additional human authority hash drift".into())}
  let a:Value=serde_json::from_slice(&fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  if a["answer"]!="追加1ジョブ・最大3turnを承認"||a["cumulativeMax"]["jobs"]!=3||a["cumulativeMax"]["turnStarts"]!=7{return Err("Additional live trial not approved".into())}maximum=(3,7);
 }
 if v["maxJobs"]!=maximum.0||v["maxTurnStarts"]!=maximum.1{return Err("Budget differs from exact human approval".into())}Ok(maximum)
}
impl LiveTrial{
 pub fn maxima(&self)->Result<(u64,u64),String>{let v:Value=serde_json::from_slice(&fs::read(&self.permit).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;limits(&v)}
 pub fn load(path:&Path)->Result<Self,String>{
  let permit=canonical(path)?;let v:Value=serde_json::from_slice(&fs::read(&permit).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  if v["jobId"]!="AI-GUILD-20261026"||v["authorized"]!=true{return Err("QA live permit does not match human authorization".into())}limits(&v)?;
  let root=canonical(Path::new(v["root"].as_str().ok_or("Missing QA root")?))?;
  if !root.join(".ai-guild-qa").is_file()||!permit.starts_with(&root){return Err("Unmarked/outside QA permit".into())}
  let authority=canonical(Path::new(v["authority"].as_str().ok_or("Missing human authority")?))?;let authority_sha=v["authoritySha256"].as_str().ok_or("Missing authority hash")?.to_string();
  if hash(&authority)?!=authority_sha{return Err("Human approval changed".into())}
  let a:Value=serde_json::from_slice(&fs::read(&authority).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  if a["answers"]["realInference"]!="隔離確認後、最大6回で承認"{return Err("Live inference not approved".into())}
  let proof_path=Path::new(v["isolationProof"].as_str().ok_or("Missing isolation proof")?);if hash(proof_path)?!=v["isolationProofSha256"].as_str().unwrap_or(""){return Err("Isolation proof hash drift".into())}let proof:Value=serde_json::from_slice(&fs::read(proof_path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  if proof["status"]!="PASS_NON_INFERENCE_ONLY"||proof["ephemeralThread"]["ephemeral"]!=true||!proof["ephemeralThread"]["path"].is_null()||proof["outsideQaRolloutsForThisThread"].as_array().is_none_or(|a|!a.is_empty()){return Err("Isolation proof is incomplete".into())}
  let sqlite=canonical(Path::new(proof["effectiveIsolation"]["sqlite_home"].as_str().ok_or("Missing effective sqlite_home")?))?;let logs=canonical(Path::new(proof["effectiveIsolation"]["log_dir"].as_str().ok_or("Missing effective log_dir")?))?;
  let phase_root=canonical(root.parent().ok_or("Missing phase root")?)?;
  if !sqlite.starts_with(&phase_root)||!logs.starts_with(&phase_root){return Err("Server storage outside marked phase QA area".into())}
  let model=proof["model"].as_str().ok_or("Current model not verified")?.to_string();
  let app=canonical(&PathBuf::from(std::env::var_os("AI_GUILD_QA_DATA_DIR").ok_or("Missing QA app data directory")?))?;
  if !app.starts_with(&root)||!app.join(".ai-guild-qa").is_file(){return Err("Live app data not isolated".into())}
  Ok(Self{permit,root,sqlite,logs,model,authority,authority_sha,connection_id:crate::model::id()})
 }
 pub fn trace(&self,kind:&str,v:&Value)->Result<(),String>{
  // Never record prompt, answers, command arguments, account data, or response text.
  let mut f=fs::OpenOptions::new().create(true).append(true).open(self.root.join("wire-metadata.jsonl")).map_err(|e|e.to_string())?;
  let p=&v["params"];let row=json!({"at":crate::model::now(),"connectionId":self.connection_id,"appPid":std::process::id(),"kind":kind,"method":v["method"],"id":v["id"],"threadId":p["threadId"],"turnId":p["turnId"],"turn":p["turn"].get("id"),"formalStatus":p["turn"].get("status"),"requestId":p["requestId"],"itemId":p["itemId"],"resultTurnId":v["result"]["turn"]["id"],"resultThreadId":v["result"]["thread"]["id"],"questionIds":p["questions"].as_array().map(|q|q.iter().map(|q|q["id"].clone()).collect::<Vec<_>>()),"secretQuestionCount":p["questions"].as_array().map(|q|q.iter().filter(|q|q["isSecret"]==true).count()),"responseAnswerIds":v["result"]["answers"].as_object().map(|a|a.keys().collect::<Vec<_>>()),"decision":v["result"].get("decision")});
  writeln!(f,"{row}").and_then(|_|f.flush()).map_err(|e|e.to_string())
 }
 pub fn prepare(&self,method:&str,params:&mut Value,request:u64)->Result<(),String>{
  if hash(&self.authority)?!=self.authority_sha{return Err("Human authorization hash drift".into())}
  if method=="initialize"{params["capabilities"]=json!({"experimentalApi":true});return Ok(())}
  if method!="thread/start"&&method!="turn/start"{return Ok(())}
  let lock_path=self.permit.with_extension("lock");let _file=fs::OpenOptions::new().create_new(true).write(true).open(&lock_path).map_err(|_|"QA permit is locked; no send attempted".to_string())?;let _lock=Lock(lock_path);
  let mut permit:Value=serde_json::from_slice(&fs::read(&self.permit).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  let maximum=limits(&permit)?;let field=if method=="thread/start"{"jobsReserved"}else{"turnsReserved"};let limit=if method=="thread/start"{maximum.0}else{maximum.1};let used=permit[field].as_u64().ok_or("Invalid QA budget")?;
  if used>=limit{return Err("Approved QA inference/job budget exhausted; no send attempted".into())}
  if method=="thread/start"{
   let cwd=canonical(Path::new(params["cwd"].as_str().ok_or("Missing thread cwd")?))?;if !cwd.starts_with(&self.root){return Err("Thread workspace outside live QA".into())}
   if params["sandbox"]!="read-only"||params["approvalPolicy"]!="on-request"{return Err("QA sandbox/approval policy changed".into())}
   params["ephemeral"]=json!(true);params["model"]=json!(self.model);params["config"]["web_search"]=json!("disabled");
   params["developerInstructions"]=json!(format!("AI GUILD Phase7 dedicated, user-approved real integration test. Existing ChatGPT authentication only. Every thread is ephemeral; host SQLite/logs/artifacts are isolated QA. Operate only inside {}. No publication, SNS, purchases, paid APIs, credential access, administrator requests, external deletion, existing-file overwrite, other projects, agents, or alternate models. Default sandbox is read-only with on-request approval. You MAY propose ONLY the explicitly requested single new QA text-file creations through commandExecution or fileChange callbacks; the native UI must approve or decline each operation. Never auto-approve or grant a session-wide permission. Never retry a declined operation or create its target another way. Use request_user_input for the requested questions when the active mode supports it. Never print or echo a secret answer. If the question tool cannot mark a secret question as isSecret=true, state that limitation and do not request a real secret. All final response files are saved by the host.",self.root.display()));
  }else{
   let text=params["input"].as_array().map(|xs|xs.iter().filter_map(|x|x["text"].as_str()).collect::<Vec<_>>().join("\n")).unwrap_or_default();
   params["collaborationMode"]=json!({"mode":if text.contains("QA_PHASE7_RESEARCH"){"plan"}else{"default"},"settings":{"model":self.model,"reasoning_effort":"low","developer_instructions":null}});
  }
  permit[field]=json!(used+1);permit["revision"]=json!(permit["revision"].as_u64().unwrap_or(0)+1);
  let claim=json!({"at":crate::model::now(),"connectionId":self.connection_id,"method":method,"rpcId":request,"claim":crate::model::id(),"outcome":"UNKNOWN_BEFORE_WRITE","countedEvenIfSendFails":true});
  permit["reservations"].as_array_mut().ok_or("Missing reservation history")?.push(claim.clone());
  let bytes=serde_json::to_vec_pretty(&permit).map_err(|e|e.to_string())?;let mut f=fs::File::create(&self.permit).map_err(|e|e.to_string())?;f.write_all(&bytes).and_then(|_|f.sync_all()).map_err(|e|e.to_string())?;
  self.trace("RESERVED",&json!({"method":method,"id":request,"params":{}}))?;Ok(())
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn budget_is_reserved_before_send_and_never_exceeds_six(){
  let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("target/test-data/live-budget-unit").join(crate::model::id());fs::create_dir_all(&root).unwrap();fs::write(root.join(".ai-guild-qa"),"OFFLINE budget unit test").unwrap();let authority=root.join("authority.txt");fs::write(&authority,"OFFLINE fixture; no actual authority to send").unwrap();let permit=root.join("permit.json");fs::write(&permit,serde_json::to_vec(&json!({"maxJobs":2,"maxTurnStarts":6,"jobsReserved":0,"turnsReserved":0,"revision":0,"reservations":[]})).unwrap()).unwrap();
  let trial=LiveTrial{permit:permit.clone(),root:root.clone(),sqlite:root.clone(),logs:root.clone(),model:"OFFLINE_NO_MODEL".into(),authority:authority.clone(),authority_sha:hash(&authority).unwrap(),connection_id:"OFFLINE".into()};
  for n in 0..6{trial.prepare("turn/start",&mut json!({"input":[]}),n).unwrap();}assert!(trial.prepare("turn/start",&mut json!({"input":[]}),7).is_err());let v:Value=serde_json::from_slice(&fs::read(permit).unwrap()).unwrap();assert_eq!(v["turnsReserved"],6);assert_eq!(v["reservations"].as_array().unwrap().len(),6);fs::write(authority,"changed").unwrap();assert!(trial.prepare("initialize",&mut json!({}),8).is_err());
 }
}
