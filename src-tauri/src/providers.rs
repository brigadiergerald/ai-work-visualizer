//! Versioned connection boundary. Manual posting lives in the separate posting module.
use crate::{engine::Engine,model::{id,now},provider_credentials::{self,Source},provider_http::{self,ReadTransport}};
use rusqlite::{params,Connection,OptionalExtension};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::collections::HashMap;

#[derive(Clone,Serialize,Deserialize,Debug,PartialEq)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Workspace{pub id:String,pub name:String}
#[derive(Clone,Serialize,Deserialize,Debug,PartialEq)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Channel{pub id:String,pub workspace_id:String,pub name:String,pub platform:String,pub queue_paused:bool}
#[derive(Clone,Serialize,Deserialize,Debug)]#[serde(rename_all="camelCase")]
pub struct ReadEvidence{pub operation:String,pub started_at:String,pub ended_at:String,pub request_sha256:String,pub http_status:Option<u32>,pub response_sha256:Option<String>,pub result:String}
#[derive(Clone,Serialize,Deserialize,Debug)]#[serde(rename_all="camelCase")]
pub struct CheckError{pub code:String,pub message:String}
impl CheckError {pub fn new(code:&str)->Self{Self{code:code.into(),message:match code{
 "AUTHENTICATION_FAILED"=>"認証に失敗しました。選択した資格情報の有効性を確認し、再接続してください。別の認証には切り替えません。",
 "FORBIDDEN"=>"アクセス権限が不足しています。サービス側の権限を確認してください。",
 "RATE_LIMITED"=>"APIの利用制限に達しました。自動再試行は行いません。",
 "TIMEOUT"|"TRANSPORT_UNKNOWN"=>"通信結果が不明です。記録を保持して停止しました。自動再送は行いません。",
 "GRAPHQL_ERROR"=>"サービスがGraphQLエラーを返しました。接続先の権限と設定を確認してください。",
 "CREDENTIAL_UNAVAILABLE"=>"選択した資格情報を利用できません。参照先やWindows資格情報を確認してください。",
 _=>"接続結果を確認できません。記録したエラーコードを確認してください。"
 }.into()}}}
#[derive(Default)]pub struct Discovery{pub authenticated:bool,pub account_id:Option<String>,pub workspaces:Vec<Workspace>,pub channels:Vec<Channel>,pub evidence:Vec<ReadEvidence>,pub error:Option<CheckError>}
pub trait ProviderAdapter {
 fn descriptor(&self)->Value;
 fn discover(&self,transport:&mut dyn ReadTransport,fields:&HashMap<String,String>)->Discovery;
}
struct Buffer;
impl ProviderAdapter for Buffer {
 fn descriptor(&self)->Value{json!({"id":"buffer","name":"Buffer","apiStyle":"GraphQL","adapterVersion":"1.2.0","availability":"IMPLEMENTED","authMethods":[{"id":"apiKey","label":"APIキー","implemented":true,"fields":[]}],"credentialModes":["existingEnv","nativeVault"],"credentialEnvField":"BUFFER_API_KEY","networks":"APIが返したSNS種別のみを使用","planLimitations":"利用者自身のBuffer契約・API利用権限に依存。個人APIキーでの確認は配布用OAuth対応を意味しません。","oauth":"DESIGN_ONLY: public client + PKCE; app登録・認可・token取得未実装","capabilities":{"testConnection":true,"listWorkspaces":true,"listChannels":true,"createDraft":false,"schedulePost":true,"publishPost":true,"getPostStatus":true},"scope":"LIVE_READ_AND_MANUAL_THREADS_POST"})}
 fn discover(&self,t:&mut dyn ReadTransport,_:&HashMap<String,String>)->Discovery{crate::provider_buffer::discover(t)}
}
struct Mock;
impl ProviderAdapter for Mock {
 fn descriptor(&self)->Value{json!({"id":"mock","name":"Mock Provider（通信なし）","apiStyle":"Offline","adapterVersion":"1.0.0","availability":"MOCK_ONLY","authMethods":[{"id":"simulation","label":"模擬認証（秘密情報不要）","implemented":true,"fields":[{"id":"scenario","label":"模擬応答","options":["success","authenticationFailure","forbidden","rateLimited","timeout","graphqlError"]}]}],"credentialModes":["mock"],"networks":"架空のSNS","planLimitations":"オフライン試験専用。実サービスへの接続ではありません。","capabilities":{"testConnection":true,"listWorkspaces":true,"listChannels":true,"createDraft":false,"schedulePost":false,"publishPost":false,"getPostStatus":false},"scope":"OFFLINE_SIMULATION"})}
 fn discover(&self,_:&mut dyn ReadTransport,f:&HashMap<String,String>)->Discovery{
  let code=match f.get("scenario").map(String::as_str).unwrap_or("success"){"success"=>None,"authenticationFailure"=>Some("AUTHENTICATION_FAILED"),"forbidden"=>Some("FORBIDDEN"),"rateLimited"=>Some("RATE_LIMITED"),"timeout"=>Some("TIMEOUT"),_=>Some("GRAPHQL_ERROR")};
  if let Some(code)=code{return Discovery{error:Some(CheckError::new(code)),..Default::default()}}
  Discovery{authenticated:true,account_id:Some("mock-account".into()),workspaces:vec![Workspace{id:"mock-workspace".into(),name:"模擬ワークスペース".into()}],channels:vec![Channel{id:"mock-channel".into(),workspace_id:"mock-workspace".into(),name:"架空の投稿先".into(),platform:"mock-social".into(),queue_paused:false}],..Default::default()}
 }
}
fn adapters()->Vec<Box<dyn ProviderAdapter>>{vec![Box::new(Buffer),Box::new(Mock)]}
fn adapter(provider:&str)->Result<Box<dyn ProviderAdapter>,String>{adapters().into_iter().find(|a|a.descriptor()["id"]==provider).ok_or("Adapterが必要です。このサービスはまだ利用できません".into())}
fn providers()->Vec<Value>{let mut v=adapters().into_iter().map(|a|a.descriptor()).collect::<Vec<_>>();for (id,name) in [("publer","Publer"),("ayrshare","Ayrshare")]{v.push(json!({"id":id,"name":name,"availability":"ADAPTER_REQUIRED","authMethods":[],"credentialModes":[],"capabilities":{},"planLimitations":"正式Adapter未実装。認証・接続は利用できません。"}))}v}

#[derive(Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase")]
pub struct Check {pub id:String,pub started_at:String,pub ended_at:Option<String>,pub scope:String,pub status:String,pub authenticated:bool,pub targets_verified:bool,pub error:Option<CheckError>,pub evidence:Vec<ReadEvidence>,pub external_writes:u32}
#[derive(Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase")]
pub struct ConnectionState {
 pub id:String,pub provider_id:String,pub revision:u64,pub credential_ref:String,pub auth_method:String,pub fields:HashMap<String,String>,pub state:String,
 pub account_id:Option<String>,pub workspaces:Vec<Workspace>,pub channels:Vec<Channel>,pub selected_workspace_id:Option<String>,pub selected_channel_ids:Vec<String>,pub checks:Vec<Check>,pub configured_at:String,
}
#[derive(Deserialize)]#[serde(tag="operation",rename_all="camelCase",rename_all_fields="camelCase",deny_unknown_fields)]
pub enum Action{
 ListProviders,Get,GetProviderCapabilities{provider_id:String},
 Connect{provider_id:String,auth_method:String,source:Source,#[serde(default)]fields:HashMap<String,String>},
 TestConnection{connection_id:String,revision:u64},Disconnect{connection_id:String,revision:u64},
 ListWorkspaces{connection_id:String},ListChannels{connection_id:String,workspace_id:String},GetConnectionStatus{connection_id:String},
 SaveTargets{connection_id:String,revision:u64,workspace_id:String,channel_ids:Vec<String>},
}
fn init(db:&Connection)->Result<(),String>{db.execute_batch("CREATE TABLE IF NOT EXISTS provider_connections(id TEXT PRIMARY KEY,revision INTEGER NOT NULL,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS provider_credential_refs(id TEXT PRIMARY KEY,provider_id TEXT NOT NULL,connection_id TEXT NOT NULL,source TEXT NOT NULL);").map_err(|_|"接続設定の保存領域を利用できません".into())}
pub(crate) fn load(db:&Connection,id:&str)->Result<ConnectionState,String>{let text:String=db.query_row("SELECT json FROM provider_connections WHERE id=?1",[id],|r|r.get(0)).optional().map_err(|_|"接続設定の読取に失敗しました")?.ok_or("接続設定がありません")?;serde_json::from_str(&text).map_err(|_|"接続設定の形式が不正です".into())}
fn put(db:&Connection,c:&mut ConnectionState)->Result<(),String>{let old=c.revision;c.revision+=1;let rows=db.execute("UPDATE provider_connections SET revision=?1,json=?2 WHERE id=?3 AND revision=?4",params![c.revision,serde_json::to_string(c).unwrap(),c.id,old]).map_err(|_|"接続設定を保存できません")?;if rows!=1{return Err("設定が更新されています。再読込してください".into())}Ok(())}
fn current(db:&Connection,id:&str,revision:u64)->Result<ConnectionState,String>{let c=load(db,id)?;if c.revision!=revision{return Err("設定が更新されています。再読込してください".into())}Ok(c)}
fn state(db:&Connection)->Result<Value,String>{let mut q=db.prepare("SELECT json FROM provider_connections ORDER BY rowid").map_err(|_|"接続設定を読み込めません")?;let rows=q.query_map([],|r|r.get::<_,String>(0)).map_err(|_|"接続設定を読み込めません")?;let mut connections=vec![];for r in rows{let c:ConnectionState=serde_json::from_str(&r.map_err(|_|"接続設定を読み込めません")?).map_err(|_|"接続設定の形式が不正です")?;connections.push(c)}Ok(json!({"version":1,"providers":providers(),"connections":connections,"externalWrites":0,"publication":"NOT_PERFORMED"}))}
struct NoNetwork;
impl ReadTransport for NoNetwork{fn query(&mut self,_:&[u8])->Result<provider_http::Reply,provider_http::Error>{Err(provider_http::Error::Transport)}}
fn finish(c:&mut ConnectionState,d:Discovery){
 let mock=c.provider_id=="mock";c.state=if let Some(e)=&d.error{match e.code.as_str(){"TIMEOUT"|"TRANSPORT_UNKNOWN"=>"UNKNOWN",s=>s}}else if mock{"MOCK_VERIFIED"}else if d.workspaces.is_empty(){"AUTHENTICATED"}else{"TARGETS_VERIFIED"}.into();
 c.account_id=d.account_id;c.workspaces=d.workspaces;c.channels=d.channels;
 // Selections survive refresh only if returned identities still belong to the same workspace.
 if !c.workspaces.iter().any(|w|Some(&w.id)==c.selected_workspace_id.as_ref()){c.selected_workspace_id=None;c.selected_channel_ids.clear();}
 c.selected_channel_ids.retain(|id|c.channels.iter().any(|ch|&ch.id==id&&Some(&ch.workspace_id)==c.selected_workspace_id.as_ref()));
 let check=c.checks.last_mut().unwrap();check.ended_at=Some(now());check.status=c.state.clone();check.authenticated=d.authenticated;check.targets_verified=d.error.is_none()&&!c.workspaces.is_empty();check.error=d.error;check.evidence=d.evidence;
}
pub fn perform(e:&std::sync::Arc<Engine>,a:Action)->Result<Value,String>{
 // Engine profile lease + same mutex as the rest of Core; CAS protects stale dashboards.
 let d=e.data.lock().map_err(|_|"Coreロックエラー")?;let db=&d.store.0;init(db)?;
 match a {
  Action::ListProviders=>Ok(json!(providers())),Action::Get=>state(db),
  Action::GetProviderCapabilities{provider_id}=>Ok(adapter(&provider_id)?.descriptor()),
  Action::Connect{provider_id,auth_method,source,fields}=>{
   let a=adapter(&provider_id)?;let meta=a.descriptor();
   if !meta["authMethods"].as_array().unwrap().iter().any(|m|m["id"]==auth_method&&m["implemented"]==true){return Err("この認証方式は未実装です".into())}
   provider_credentials::validate_source(&provider_id,&source)?;
   if provider_id=="buffer"&&!fields.is_empty(){return Err("未対応の認証項目です".into())}
   if provider_id=="mock"&&(fields.keys().any(|k|k!="scenario")||fields.get("scenario").is_some_and(|s|!["success","authenticationFailure","forbidden","rateLimited","timeout","graphqlError"].contains(&s.as_str()))){return Err("模擬設定が不正です".into())}
   let cid=id();let reference=format!("ai-guild/provider/{provider_id}/{cid}");
   if matches!(source,Source::NativeVault){provider_credentials::enroll_vault(&reference)?;}
   let c=ConnectionState{id:cid,provider_id:provider_id.clone(),revision:1,credential_ref:reference,auth_method,fields,state:"CONFIGURED".into(),account_id:None,workspaces:vec![],channels:vec![],selected_workspace_id:None,selected_channel_ids:vec![],checks:vec![],configured_at:now()};
   let tx=db.unchecked_transaction().map_err(|_|"接続設定を保存できません")?;
   tx.execute("INSERT INTO provider_connections VALUES(?1,?2,?3)",params![c.id,c.revision,serde_json::to_string(&c).unwrap()]).map_err(|_|"接続設定を保存できません")?;
   tx.execute("INSERT INTO provider_credential_refs VALUES(?1,?2,?3,?4)",params![c.credential_ref,provider_id,c.id,serde_json::to_string(&source).unwrap()]).map_err(|_|"資格情報参照を保存できません")?;tx.commit().map_err(|_|"接続設定を保存できません")?;Ok(json!(c))
  },
  Action::TestConnection{connection_id,revision}=>{
   if crate::posting::connection_busy(db,&connection_id){return Err("POST_SUBMISSION_IN_FLIGHT".into())}
   let mut c=current(db,&connection_id,revision)?;
   if ["DISCONNECTED","CHECKING_UNKNOWN","UNKNOWN"].contains(&c.state.as_str()){return Err("切断済みまたは結果不明です。元の記録を確認し、接続設定を見直してください。自動再送しません".into())}
   let a=adapter(&c.provider_id)?;
   let source:String=db.query_row("SELECT source FROM provider_credential_refs WHERE id=?1 AND provider_id=?2 AND connection_id=?3",params![c.credential_ref,c.provider_id,c.id],|r|r.get(0)).map_err(|_|"Providerに対応する資格情報参照がありません")?;
   let source:Source=serde_json::from_str(&source).map_err(|_|"資格情報参照が不正です")?;
   c.checks.push(Check{id:id(),started_at:now(),ended_at:None,scope:if c.provider_id=="buffer"{"LIVE_READ_ONLY".into()}else{a.descriptor()["scope"].as_str().unwrap().into()},status:"UNKNOWN".into(),authenticated:false,targets_verified:false,error:None,evidence:vec![],external_writes:0});c.state="CHECKING_UNKNOWN".into();put(db,&mut c)?;
   // Intent is durable before any credential resolution or request. No retries.
   let discovery=if c.provider_id=="mock" {a.discover(&mut NoNetwork,&c.fields)}else{
    match provider_credentials::resolve(&c.provider_id,&c.credential_ref,&source){Ok(secret)=>a.discover(&mut provider_http::BufferTransport::new(secret),&c.fields),Err(_)=>Discovery{error:Some(CheckError::new("CREDENTIAL_UNAVAILABLE")),..Default::default()}}
   };finish(&mut c,discovery);put(db,&mut c)?;Ok(json!(c))
  },
  Action::SaveTargets{connection_id,revision,workspace_id,channel_ids}=>{
   if crate::posting::connection_busy(db,&connection_id){return Err("POST_SUBMISSION_IN_FLIGHT".into())}
   let mut c=current(db,&connection_id,revision)?;
   if !["TARGETS_VERIFIED","MOCK_VERIFIED"].contains(&c.state.as_str())||!c.workspaces.iter().any(|w|w.id==workspace_id){return Err("取得済みのワークスペースを選択してください".into())}
   let mut unique=std::collections::HashSet::new();
   if channel_ids.iter().any(|id|!unique.insert(id)||!c.channels.iter().any(|ch|&ch.id==id&&ch.workspace_id==workspace_id)){return Err("投稿先のProvider・Workspaceが一致しません".into())}
   c.selected_workspace_id=Some(workspace_id);c.selected_channel_ids=channel_ids;put(db,&mut c)?;Ok(json!(c))
  },
  Action::Disconnect{connection_id,revision}=>{if crate::posting::connection_busy(db,&connection_id){return Err("POST_SUBMISSION_IN_FLIGHT".into())}let mut c=current(db,&connection_id,revision)?;c.state="DISCONNECTED".into();c.selected_workspace_id=None;c.selected_channel_ids.clear();put(db,&mut c)?;Ok(json!(c))},
  Action::GetConnectionStatus{connection_id}=>Ok(json!(load(db,&connection_id)?)),
  Action::ListWorkspaces{connection_id}=>Ok(json!(load(db,&connection_id)?.workspaces)),
  Action::ListChannels{connection_id,workspace_id}=>Ok(json!(load(db,&connection_id)?.channels.into_iter().filter(|c|c.workspace_id==workspace_id).collect::<Vec<_>>())),
 }
}

// Future data contracts only. No write method is callable from Core actions.
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum PostState{Prepared,ApprovalRequired,Approved,SubmissionStarted,ProviderAccepted,Scheduled,PublishedVerified,Failed,Unknown}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PostBinding{pub job_id:String,pub provider_id:String,pub connection_id:String,pub workspace_id:String,pub channel_id:String,pub platform:String,pub text:String,pub text_sha256:String,pub method:String,pub scheduled_at:Option<String>}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase")]
pub struct PublicationApproval{pub approval_id:String,pub binding:PostBinding}
impl PublicationApproval{pub fn matches(&self,current:&PostBinding)->bool{use sha2::{Digest,Sha256};!self.approval_id.is_empty()&&self.binding==*current&&format!("{:x}",Sha256::digest(current.text.as_bytes()))==current.text_sha256}}
#[derive(Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase")]
pub struct FuturePost{pub binding:PostBinding,pub publication_approval:Option<PublicationApproval>,pub state:PostState,pub external_receipt_id:Option<String>,pub provider_status:Option<String>,pub publication_check:String,pub error:Option<CheckError>,pub submission_record_id:Option<String>,pub provider_idempotency_key:Option<String>}
