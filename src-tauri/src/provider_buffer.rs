//! Buffer protocol stays inside this adapter. Only two fixed read documents exist.
use crate::{providers::{Workspace,Channel,Discovery,CheckError,ReadEvidence},provider_http::{ReadTransport,Error}};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
pub const ACCOUNT:&str="query BufferReadOnlyAccount { account { id organizations { id name } } }";
pub fn channels_query(id:&str)->Result<String,CheckError>{
 if id.is_empty()||id.len()>128||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_'){return Err(CheckError::new("INVALID_RESPONSE"))}
 Ok(format!("query BufferReadOnlyChannels {{ channels(input: {{ organizationId: {} }}) {{ id name displayName service isQueuePaused }} }}",json!(id)))
}
pub fn approved_body(body:&[u8])->bool {
 let Ok(v)=serde_json::from_slice::<Value>(body) else{return false};
 if v.as_object().is_none_or(|o|o.len()!=1){return false}
 let Some(q)=v["query"].as_str() else{return false};if q==ACCOUNT{return true}
 let prefix="query BufferReadOnlyChannels { channels(input: { organizationId: ";
 let suffix=" }) { id name displayName service isQueuePaused } }";
 let Some(id)=q.strip_prefix(prefix).and_then(|s|s.strip_suffix(suffix)) else{return false};
 let Ok(id)=serde_json::from_str::<String>(id) else{return false};channels_query(&id).is_ok_and(|s|s==q)
}
fn read(t:&mut dyn ReadTransport,document:String,evidence:&mut Vec<ReadEvidence>)->Result<Value,CheckError>{
 let body=serde_json::to_vec(&json!({"query":document})).unwrap();
 if !approved_body(&body){return Err(CheckError::new("UNSUPPORTED"))}
 let started=crate::model::now();let request_sha256=format!("{:x}",Sha256::digest(&body));
 let reply=t.query(&body);
 let mut item=ReadEvidence{started_at:started,ended_at:crate::model::now(),request_sha256,http_status:None,response_sha256:None,operation:if document==ACCOUNT{"account"}else{"channels"}.into(),result:"UNKNOWN".into()};
 let result=(||{
  let r=reply.map_err(|e|CheckError::new(match e{Error::Timeout=>"TIMEOUT",Error::TooLarge=>"RESPONSE_TOO_LARGE",Error::Transport=>"TRANSPORT_UNKNOWN"}))?;
  item.http_status=Some(r.status);
  if r.status!=200{return Err(CheckError::new(match r.status{401=>"AUTHENTICATION_FAILED",403=>"FORBIDDEN",429=>"RATE_LIMITED",300..=399=>"REDIRECT_REJECTED",_=>"HTTP_FAILED"}))}
  item.response_sha256=Some(format!("{:x}",Sha256::digest(&r.bytes)));
  let v:Value=serde_json::from_slice(&r.bytes).map_err(|_|CheckError::new("INVALID_RESPONSE"))?;
  // Never expose arbitrary GraphQL error text or partial data as success.
  if v.get("errors").is_some_and(|x|!x.is_null()&&x.as_array().is_none_or(|a|!a.is_empty())){return Err(CheckError::new("GRAPHQL_ERROR"))}
  if v["data"].is_null(){return Err(CheckError::new("INVALID_RESPONSE"))}Ok(v["data"].clone())
 })();
 item.result=result.as_ref().map(|_|"PASS").unwrap_or_else(|e|e.code.as_str()).into();evidence.push(item);result
}
fn string(v:&Value,key:&str)->Result<String,CheckError>{let s=v[key].as_str().ok_or_else(||CheckError::new("INVALID_RESPONSE"))?;if s.is_empty()||s.len()>512||s.chars().any(char::is_control){return Err(CheckError::new("INVALID_RESPONSE"))}Ok(s.into())}
pub fn discover(t:&mut dyn ReadTransport)->Discovery{
 let mut d=Discovery::default();
 let result=(||{
  let a=read(t,ACCOUNT.into(),&mut d.evidence)?;let a=&a["account"];d.account_id=Some(string(a,"id")?);d.authenticated=true;
  let orgs=a["organizations"].as_array().ok_or_else(||CheckError::new("INVALID_RESPONSE"))?;
  if orgs.len()>20{return Err(CheckError::new("LIMIT_EXCEEDED"))}
  let mut ids=std::collections::HashSet::new();
  for o in orgs{
   let id=string(o,"id")?;if !ids.insert(id.clone()){return Err(CheckError::new("INVALID_RESPONSE"))}
   d.workspaces.push(Workspace{id,name:string(o,"name")?});
  }
  for w in &d.workspaces{
   let q=channels_query(&w.id)?;let c=read(t,q,&mut d.evidence)?;
   let channels=c["channels"].as_array().ok_or_else(||CheckError::new("INVALID_RESPONSE"))?;
   if channels.len()>500{return Err(CheckError::new("LIMIT_EXCEEDED"))}
   let mut seen=std::collections::HashSet::new();
   for c in channels{
    let id=string(c,"id")?;if !seen.insert(id.clone()){return Err(CheckError::new("INVALID_RESPONSE"))}
    d.channels.push(Channel{id,workspace_id:w.id.clone(),name:string(c,"name")?,platform:string(c,"service")?,queue_paused:c["isQueuePaused"].as_bool().ok_or_else(||CheckError::new("INVALID_RESPONSE"))?});
   }
  }Ok(())
 })();
 if let Err(e)=result{d.error=Some(e);d.channels.clear();}d
}
