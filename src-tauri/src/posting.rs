//! Independent manual publication ledger. Never changes production Job/Task meaning.
use crate::{engine::Engine,model::{id,now,Job},providers::{self,ConnectionState},posting_wire::{self as wire,Transport}};
use rusqlite::{Connection,params,OptionalExtension};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{sync::Arc,fs,path::Path};
#[derive(Clone,Copy,Default,Serialize,Deserialize,PartialEq)]#[serde(rename_all="camelCase")]
pub enum DeliveryMode{#[default]Scheduled,Immediate}
#[derive(Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase")]
pub struct Record{
 #[serde(default)]pub delivery_mode:DeliveryMode,
 pub id:String,pub revision:u64,pub job_id:String,pub connection_id:String,pub connection_revision:u64,pub account_id:String,pub workspace_id:String,pub channel_id:String,pub channel_name:String,
 pub text:String,pub text_sha256:String,pub source_sha256:String,pub due_at:String,pub time_zone:String,pub created_at:String,pub expires_at:String,pub review_token:String,
 pub state:String,pub approval_id:Option<String>,pub submission_id:Option<String>,pub receipt_id:Option<String>,pub provider_status:Option<String>,pub external_link:Option<String>,pub error:Option<String>,pub history:Vec<Value>,
}
#[derive(Deserialize)]#[serde(tag="operation",rename_all="camelCase",rename_all_fields="camelCase",deny_unknown_fields)]
pub enum Action{Get{job_id:String},Prepare{job_id:String,connection_id:String,connection_revision:u64,channel_id:String,due_at:String},PrepareNow{job_id:String,connection_id:String,connection_revision:u64,channel_id:String},Submit{record_id:String,revision:u64,review_token:String},Check{record_id:String,revision:u64}}
fn init(db:&Connection)->Result<(),String>{db.execute_batch("CREATE TABLE IF NOT EXISTS manual_posts(id TEXT PRIMARY KEY,job_id TEXT NOT NULL,connection_id TEXT NOT NULL,revision INTEGER NOT NULL,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS manual_post_guards(source_key TEXT PRIMARY KEY,request_key TEXT UNIQUE NOT NULL,record_id TEXT UNIQUE NOT NULL);").map_err(|_|"POST_STORAGE".to_string())}
pub fn connection_busy(db:&Connection,cid:&str)->bool{
 let exists:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='manual_posts')",[],|r|r.get(0)).unwrap_or(true);if !exists{return false}
 db.prepare("SELECT json FROM manual_posts WHERE connection_id=?1").and_then(|mut q|q.query_map([cid],|r|r.get::<_,String>(0)).and_then(|rows|rows.collect::<Result<Vec<_>,_>>())).map(|rows|rows.iter().any(|s|serde_json::from_str::<Record>(s).map(|r|r.state=="SUBMISSION_STARTED").unwrap_or(true))).unwrap_or(true)
}
fn get(db:&Connection,rid:&str)->Result<Record,String>{let raw:String=db.query_row("SELECT json FROM manual_posts WHERE id=?1",[rid],|r|r.get(0)).map_err(|_|"POST_MISSING")?;serde_json::from_str(&raw).map_err(|_|"POST_RECORD_INVALID".into())}
fn put(db:&Connection,r:&mut Record,event:&str)->Result<(),String>{let old=r.revision;r.revision+=1;r.history.push(json!({"at":now(),"event":event,"state":r.state,"error":r.error,"receiptId":r.receipt_id}));let n=db.execute("UPDATE manual_posts SET revision=?1,json=?2 WHERE id=?3 AND revision=?4",params![r.revision,serde_json::to_string(r).unwrap(),r.id,old]).map_err(|_|"POST_SAVE")?;if n!=1{return Err("POST_STALE".into())}Ok(())}
fn source(e:&Engine,jid:&str)->Result<(Job,String,String),String>{
 let s=e.snapshot();if s.mode!="real"{return Err("POST_REAL_MODE_REQUIRED".into())}let j=s.job.iter().chain(s.history.iter()).find(|j|j.job_id==jid).ok_or("POST_JOB_MISSING")?;
 if j.status!="completed"||j.mode!="real"||!crate::sns::applies(j){return Err("POST_JOB_NOT_DELIVERED".into())}
 let q=crate::practical::verify_product(j)?;if j.quality.as_ref().is_none_or(|old|old.machine_status!="PASS"||old.artifact_sha256!=q.artifact_sha256){return Err("POST_QUALITY_MISMATCH".into())}crate::sns::approved(j)?;
 let a=j.artifacts.iter().find(|a|a.name=="post.txt"&&a.kind=="product").ok_or("POST_SOURCE_MISSING")?;crate::engine::verify_artifact(a,Path::new(&j.workspace))?;
 let bytes=fs::read(&a.path).map_err(|_|"POST_SOURCE_READ")?;if crate::purposes::sha256(&bytes)!=a.sha256{return Err("POST_SOURCE_CHANGED".into())}let text=String::from_utf8(bytes).map_err(|_|"POST_SOURCE_ENCODING")?;
 if text.trim().is_empty()||text.chars().count()>500{return Err("POST_TEXT_LENGTH".into())}Ok((j.clone(),text,a.sha256.clone()))
}
fn connection(e:&Engine,cid:&str,rev:Option<u64>,channel:&str)->Result<ConnectionState,String>{
 let d=e.data.lock().map_err(|_|"POST_LOCK")?;let c=providers::load(&d.store.0,cid)?;
 if c.provider_id!="buffer"||c.state!="TARGETS_VERIFIED"||rev.is_some_and(|v|v!=c.revision)||c.selected_workspace_id.is_none()||!c.selected_channel_ids.iter().any(|id|id==channel)||!c.channels.iter().any(|ch|ch.id==channel&&ch.platform=="threads"&&Some(&ch.workspace_id)==c.selected_workspace_id.as_ref())||c.account_id.is_none(){return Err("POST_TARGET_NOT_SELECTED".into())}Ok(c)
}
fn secret(e:&Engine,c:&ConnectionState)->Result<crate::provider_credentials::Secret,String>{
 let d=e.data.lock().map_err(|_|"POST_LOCK")?;let raw:String=d.store.0.query_row("SELECT source FROM provider_credential_refs WHERE id=?1 AND provider_id='buffer' AND connection_id=?2",params![c.credential_ref,c.id],|r|r.get(0)).map_err(|_|"POST_CREDENTIAL_REFERENCE")?;
 let src=serde_json::from_str(&raw).map_err(|_|"POST_CREDENTIAL_REFERENCE")?;crate::provider_credentials::resolve("buffer",&c.credential_ref,&src)
}
fn request(t:&mut dyn Transport,q:&str,v:Value)->Result<Value,String>{
 let body=wire::body(q,v);if !wire::approved_body(&body){return Err("POST_WIRE_REJECTED".into())}
 let reply=t.exchange(&body).map_err(|_|"POST_TRANSPORT_UNKNOWN")?;if reply.status!=200{return Err(format!("POST_HTTP_{}",reply.status))}
 let value:Value=serde_json::from_slice(&reply.bytes).map_err(|_|"POST_RESPONSE_INVALID")?;if value.get("errors").is_some_and(|e|!e.is_null()&&e.as_array().is_none_or(|a|!a.is_empty()))||value["data"].is_null(){return Err("POST_GRAPHQL_ERROR".into())}Ok(value["data"].clone())
}
fn target(t:&mut dyn Transport,c:&ConnectionState,cid:&str)->Result<String,String>{
 let v=request(t,wire::TARGET,json!({"channelId":cid}))?;let ch=&v["channel"];
 if v["account"]["id"]!=c.account_id.clone().unwrap_or_default()||ch["id"]!=cid||ch["organizationId"]!=c.selected_workspace_id.clone().unwrap_or_default()||ch["service"]!="threads"||ch["isDisconnected"]!=false||ch["isLocked"]!=false||ch["isQueuePaused"]!=false||!ch["allowedActions"].as_array().is_some_and(|a|a.iter().any(|a|a=="scheduleUpdates")){return Err("POST_TARGET_NOT_READY".into())}
 let name=ch["name"].as_str().filter(|s|!s.is_empty()&&s.len()<512).ok_or("POST_TARGET_INVALID")?;Ok(name.into())
}
fn due(raw:&str)->Result<String,String>{let dt=chrono::DateTime::parse_from_rfc3339(raw).map_err(|_|"POST_TIME_INVALID")?.with_timezone(&chrono::Utc);if dt<=chrono::Utc::now()+chrono::Duration::minutes(5){return Err("POST_TIME_TOO_SOON".into())}Ok(dt.to_rfc3339_opts(chrono::SecondsFormat::Millis,true))}
fn same_time(a:&Value,b:&str)->bool{a.as_str().and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).zip(chrono::DateTime::parse_from_rfc3339(b).ok()).is_some_and(|(a,b)|a==b)}
fn list(t:&mut dyn Transport,r:&Record)->Result<Vec<Value>,String>{
 let mut rows=vec![];let mut cursor=Value::Null;let mut cursors=std::collections::HashSet::new();
 for _ in 0..10{let query=if r.delivery_mode==DeliveryMode::Immediate{wire::LIST_NOW}else{wire::LIST};let v=request(t,query,json!({"organizationId":r.workspace_id,"channelId":r.channel_id,"after":cursor}))?;let p=&v["posts"];let edges=p["edges"].as_array().ok_or("POST_LIST_INVALID")?;for edge in edges{let n=&edge["node"];if n["channelId"]!=r.channel_id{return Err("POST_LIST_WRONG_TARGET".into())}rows.push(n.clone())}
  if p["pageInfo"]["hasNextPage"]==false{return Ok(rows)}
  cursor=p["pageInfo"]["endCursor"].clone();let c=cursor.as_str().filter(|s|!s.is_empty()).ok_or("POST_LIST_CURSOR")?;if !cursors.insert(c.to_string()){return Err("POST_LIST_CURSOR".into())}
 }Err("POST_LIST_LIMIT".into())
}
fn match_post(r:&Record,p:&Value)->bool{
 if p["channelId"]!=r.channel_id||p["text"]!=r.text||!p["id"].as_str().is_some_and(|s|!s.is_empty()){return false}
 if r.delivery_mode==DeliveryMode::Scheduled{return same_time(&p["dueAt"],&r.due_at)}
 // The provider chooses dueAt for shareNow. Match its explicit mode and creation
 // window instead of inventing a requested publication timestamp.
 if p["shareMode"]!="shareNow"{return false}
 let Some(created)=p["createdAt"].as_str().and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok())else{return false};
 let Ok(start)=chrono::DateTime::parse_from_rfc3339(&r.created_at)else{return false};let Ok(end)=chrono::DateTime::parse_from_rfc3339(&r.expires_at)else{return false};created>=start&&created<=end+chrono::Duration::minutes(5)
}
fn apply_result(r:&mut Record,p:&Value)->Result<(),String>{
 if !match_post(r,p)||r.receipt_id.as_ref().is_some_and(|id|p["id"]!=*id){return Err("POST_RECEIPT_MISMATCH".into())}
 r.receipt_id=Some(p["id"].as_str().unwrap().into());r.provider_status=p["status"].as_str().map(String::from);
 r.external_link=p["externalLink"].as_str().filter(|s|["https://www.threads.net/","https://threads.net/","https://www.threads.com/","https://threads.com/"].iter().any(|p|s.starts_with(p))).map(String::from);
 r.state=match p["status"].as_str(){Some("scheduled") if r.delivery_mode==DeliveryMode::Scheduled=>"SCHEDULED",Some("scheduled"|"sending")=>"PUBLISHING",Some("sent")=>"PUBLISHED_REPORTED",Some("error")=>"FAILED",_=>"UNKNOWN"}.into();Ok(())
}
fn guard_keys(r:&Record)->(String,String){let timing=if r.delivery_mode==DeliveryMode::Immediate{"immediate"}else{&r.due_at};(crate::purposes::sha256(format!("{}\0{}\0{}",r.job_id,r.source_sha256,r.channel_id).as_bytes()),crate::purposes::sha256(format!("{}\0{}\0{}",r.channel_id,r.text_sha256,timing).as_bytes()))}
pub fn perform(e:&Arc<Engine>,a:Action)->Result<Value,String>{perform_using(e,a,None)}
pub(crate) fn perform_using(e:&Arc<Engine>,a:Action,provided:Option<&mut dyn Transport>)->Result<Value,String>{
 {let d=e.data.lock().map_err(|_|"POST_LOCK")?;init(&d.store.0)?;}
 if let Action::Get{job_id}=&a{
  let d=e.data.lock().map_err(|_|"POST_LOCK")?;let mut q=d.store.0.prepare("SELECT json FROM manual_posts WHERE job_id=?1 ORDER BY rowid DESC").map_err(|_|"POST_READ")?;let rows=q.query_map([job_id],|r|r.get::<_,String>(0)).map_err(|_|"POST_READ")?.map(|r|r.map_err(|_|"POST_READ").and_then(|raw|serde_json::from_str::<Record>(&raw).map_err(|_|"POST_RECORD_INVALID"))).collect::<Result<Vec<_>,_>>()?;
  return Ok(json!({"version":1,"jobId":job_id,"posts":rows,"scope":"MANUAL_THREADS_SCHEDULE_OR_IMMEDIATE"}))
 }
 let (mut r,c,is_prepare)=match &a{
  Action::Prepare{job_id,connection_id,connection_revision,channel_id,..}|Action::PrepareNow{job_id,connection_id,connection_revision,channel_id}=>{
   let(_,text,hash)=source(e,job_id)?;let c=connection(e,connection_id,Some(*connection_revision),channel_id)?;let (delivery_mode,scheduled)=if let Action::Prepare{due_at,..}=&a{(DeliveryMode::Scheduled,due(due_at)?)}else{(DeliveryMode::Immediate,String::new())};
   let r=Record{delivery_mode,id:id(),revision:1,job_id:job_id.clone(),connection_id:c.id.clone(),connection_revision:c.revision,account_id:c.account_id.clone().unwrap(),workspace_id:c.selected_workspace_id.clone().unwrap(),channel_id:channel_id.clone(),channel_name:String::new(),text_sha256:crate::purposes::sha256(text.as_bytes()),text,source_sha256:hash,due_at:scheduled,time_zone:"Asia/Tokyo".into(),created_at:now(),expires_at:(chrono::Utc::now()+chrono::Duration::minutes(15)).to_rfc3339(),review_token:id(),state:"REVIEW_REQUIRED".into(),approval_id:None,submission_id:None,receipt_id:None,provider_status:None,external_link:None,error:None,history:vec![]};(r,c,true)
  },
  Action::Submit{record_id,revision,..}|Action::Check{record_id,revision}=>{
   let d=e.data.lock().map_err(|_|"POST_LOCK")?;let r=get(&d.store.0,record_id)?;if r.revision!=*revision{return Err("POST_STALE".into())}drop(d);
   if let Action::Submit{review_token,..}=&a{if r.state!="REVIEW_REQUIRED"||r.review_token!=*review_token||chrono::DateTime::parse_from_rfc3339(&r.expires_at).map_err(|_|"POST_RECORD_INVALID")?<chrono::Utc::now(){return Err("POST_APPROVAL_EXPIRED_OR_USED".into())}if r.delivery_mode==DeliveryMode::Scheduled{due(&r.due_at)?;}let(_,text,hash)=source(e,&r.job_id)?;if text!=r.text||hash!=r.source_sha256{return Err("POST_SOURCE_CHANGED".into())}}
   let c=if matches!(&a,Action::Submit{..}){connection(e,&r.connection_id,Some(r.connection_revision),&r.channel_id)?}else{let d=e.data.lock().map_err(|_|"POST_LOCK")?;providers::load(&d.store.0,&r.connection_id)?};
   if c.account_id.as_ref()!=Some(&r.account_id)||c.provider_id!="buffer"{return Err("POST_ACCOUNT_CHANGED".into())}(r,c,false)
  },Action::Get{..}=>unreachable!()
 };
 let mut live;let t:&mut dyn Transport=if let Some(t)=provided{t}else{live=wire::Buffer(secret(e,&c)?);&mut live};
 if matches!(&a,Action::Prepare{..}|Action::PrepareNow{..}|Action::Submit{..}){r.channel_name=target(t,&c,&r.channel_id)?;}
 if is_prepare{
  let d=e.data.lock().map_err(|_|"POST_LOCK")?;let key=guard_keys(&r).0;let prior:Option<String>=d.store.0.query_row("SELECT record_id FROM manual_post_guards WHERE source_key=?1 OR request_key=?2",params![key,guard_keys(&r).1],|r|r.get(0)).optional().map_err(|_|"POST_READ")?;
  if let Some(id)=prior{return Ok(json!({"post":get(&d.store.0,&id)?}))}
  let latest=providers::load(&d.store.0,&c.id)?;if latest.revision!=c.revision{return Err("POST_TARGET_CHANGED".into())}
  r.history.push(json!({"at":now(),"event":"PREPARED","state":"REVIEW_REQUIRED"}));d.store.0.execute("INSERT INTO manual_posts VALUES(?1,?2,?3,?4,?5)",params![r.id,r.job_id,r.connection_id,r.revision,serde_json::to_string(&r).unwrap()]).map_err(|_|"POST_SAVE")?;return Ok(json!({"post":r}))
 }
 if matches!(&a,Action::Submit{..}){
  let rows=list(t,&r)?;if rows.iter().any(|p|p["text"]==r.text&&p["channelId"]==r.channel_id&&(r.delivery_mode==DeliveryMode::Immediate||same_time(&p["dueAt"],&r.due_at))){return Err("POST_ALREADY_EXISTS".into())}
  if r.delivery_mode==DeliveryMode::Scheduled{due(&r.due_at)?;}if chrono::DateTime::parse_from_rfc3339(&r.expires_at).map_err(|_|"POST_RECORD_INVALID")?<chrono::Utc::now(){return Err("POST_APPROVAL_EXPIRED_OR_USED".into())}
  let(_,text,hash)=source(e,&r.job_id)?;if text!=r.text||hash!=r.source_sha256{return Err("POST_SOURCE_CHANGED".into())}
  {let d=e.data.lock().map_err(|_|"POST_LOCK")?;let current=get(&d.store.0,&r.id)?;if current.revision!=r.revision||current.state!="REVIEW_REQUIRED"||providers::load(&d.store.0,&c.id)?.revision!=r.connection_revision{return Err("POST_STALE".into())}
   let tx=d.store.0.unchecked_transaction().map_err(|_|"POST_TRANSACTION")?;let keys=guard_keys(&r);tx.execute("INSERT INTO manual_post_guards VALUES(?1,?2,?3)",params![keys.0,keys.1,r.id]).map_err(|_|"POST_ALREADY_SUBMITTED_NO_RETRY")?;
   r.approval_id=Some(id());r.submission_id=Some(id());r.state="SUBMISSION_STARTED".into();put(&tx,&mut r,"USER_APPROVED_AND_RESERVED_ONCE")?;tx.commit().map_err(|_|"POST_SAVE")?;
  }
  // Durable claim above is never reused, including timeout/crash before response.
  let result=if r.delivery_mode==DeliveryMode::Immediate{request(t,wire::CREATE_NOW,json!({"input":{"channelId":r.channel_id,"text":r.text,"schedulingType":"automatic","mode":"shareNow"}}))}else{request(t,wire::CREATE,json!({"input":{"channelId":r.channel_id,"text":r.text,"schedulingType":"automatic","mode":"customScheduled","dueAt":r.due_at}}))};
  match result{
   Ok(v)=>{let p=&v["createPost"];if p["__typename"]=="PostActionSuccess"&&match_post(&r,&p["post"]){r.receipt_id=p["post"]["id"].as_str().map(String::from);r.provider_status=p["post"]["status"].as_str().map(String::from);r.state="PROVIDER_ACCEPTED".into();}else{r.state="UNKNOWN".into();r.error=Some("POST_PROVIDER_RESPONSE_NOT_VERIFIED".into());}},
   Err(error)=>{r.state="UNKNOWN".into();r.error=Some(error);}
  }
  let d=e.data.lock().map_err(|_|"POST_LOCK")?;put(&d.store.0,&mut r,"SUBMISSION_RESULT")?;return Ok(json!({"post":r}))
 }
 if r.submission_id.is_none(){return Err("POST_NOT_SUBMITTED".into())}
 if r.state=="SUBMISSION_STARTED"&&r.history.last().and_then(|h|h["at"].as_str()).and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).is_some_and(|at|chrono::Utc::now().signed_duration_since(at).num_seconds()<150){return Err("POST_SUBMISSION_IN_FLIGHT".into())}
 // Reconciliation is read-only. It never invokes createPost or modifies the source.
 let result=(||{let v=request(t,wire::TARGET,json!({"channelId":r.channel_id}))?;if v["account"]["id"]!=r.account_id||v["channel"]["id"]!=r.channel_id||v["channel"]["organizationId"]!=r.workspace_id{return Err("POST_ACCOUNT_CHANGED".into())}
  let p=if let Some(id)=&r.receipt_id{request(t,if r.delivery_mode==DeliveryMode::Immediate{wire::POST_NOW}else{wire::POST},json!({"id":id}))?["post"].clone()}else{let rows=list(t,&r)?;let matches=rows.into_iter().filter(|p|match_post(&r,p)).collect::<Vec<_>>();if matches.len()!=1{return Err("POST_UNKNOWN_NO_UNIQUE_MATCH".into())}matches[0].clone()};apply_result(&mut r,&p)
 })();
 match result{Ok(())=>r.error=None,Err(error)=>{r.state="UNKNOWN".into();r.error=Some(error);}}
 let d=e.data.lock().map_err(|_|"POST_LOCK")?;put(&d.store.0,&mut r,"READ_ONLY_RECONCILIATION")?;Ok(json!({"post":r}))
}
