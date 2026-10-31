use crate::{core_api,engine::Engine,providers::{self,Action,PostBinding,PublicationApproval},provider_credentials::{Source,Secret},provider_buffer,provider_http::{ReadTransport,Reply,Error}};
use serde_json::{json,Value};
use std::{collections::HashMap,fs,path::PathBuf};
fn root()->PathBuf{let p=std::env::var_os("AI_GUILD_TEST_ROOT").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("target/test-data/providers")).join(crate::model::id());fs::create_dir_all(&p).unwrap();p}
fn call(e:&std::sync::Arc<Engine>,v:Value)->Result<Value,String>{core_api::perform(e,serde_json::from_value(json!({"action":"providers","request":v})).map_err(|e|e.to_string())?)}
fn mock(e:&std::sync::Arc<Engine>,scenario:&str)->Value{call(e,json!({"operation":"connect","providerId":"mock","authMethod":"simulation","source":{"kind":"mock"},"fields":{"scenario":scenario}})).unwrap()}
#[test]fn provider_mock_extension_persistence_cas_and_identity_isolation(){
 let p=root();let e=Engine::open(&p,None).unwrap();let before=serde_json::to_value(e.snapshot()).unwrap();
 let list=call(&e,json!({"operation":"listProviders"})).unwrap();assert_eq!(list.as_array().unwrap().len(),4);assert_eq!(list[0]["capabilities"]["publishPost"],true);assert_eq!(list[0]["capabilities"]["schedulePost"],true);assert_eq!(list[1]["capabilities"]["publishPost"],false);assert_eq!(list[1]["authMethods"][0]["id"],"simulation");
 let c=mock(&e,"success");let id=c["id"].as_str().unwrap();let other=mock(&e,"success");
 let result=call(&e,json!({"operation":"testConnection","connectionId":id,"revision":1})).unwrap();assert_eq!(result["state"],"MOCK_VERIFIED");assert_eq!(result["checks"][0]["scope"],"OFFLINE_SIMULATION");assert_eq!(result["checks"][0]["evidence"],json!([]));
 assert!(call(&e,json!({"operation":"saveTargets","connectionId":id,"revision":1,"workspaceId":"mock-workspace","channelIds":["mock-channel"]})).is_err());
 assert!(call(&e,json!({"operation":"saveTargets","connectionId":id,"revision":3,"workspaceId":"foreign","channelIds":["mock-channel"]})).is_err());
 assert!(call(&e,json!({"operation":"saveTargets","connectionId":other["id"],"revision":1,"workspaceId":"mock-workspace","channelIds":["mock-channel"]})).is_err());
 let selected=call(&e,json!({"operation":"saveTargets","connectionId":id,"revision":3,"workspaceId":"mock-workspace","channelIds":["mock-channel"]})).unwrap();
 let status=call(&e,json!({"operation":"get"})).unwrap();assert!(!status.to_string().contains(".env"));assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);
 drop(e);let e=Engine::open(&p,None).unwrap();let recovered=call(&e,json!({"operation":"getConnectionStatus","connectionId":id})).unwrap();assert_eq!(recovered,selected);
 let disconnected=call(&e,json!({"operation":"disconnect","connectionId":id,"revision":4})).unwrap();assert_eq!(disconnected["state"],"DISCONNECTED");assert!(call(&e,json!({"operation":"testConnection","connectionId":id,"revision":5})).is_err());
}
#[test]fn provider_mock_failures_are_durable_and_unknown_stops(){
 let e=Engine::open(&root(),None).unwrap();for (scenario,expected) in [("authenticationFailure","AUTHENTICATION_FAILED"),("forbidden","FORBIDDEN"),("rateLimited","RATE_LIMITED"),("timeout","UNKNOWN"),("graphqlError","GRAPHQL_ERROR")]{let c=mock(&e,scenario);let v=call(&e,json!({"operation":"testConnection","connectionId":c["id"],"revision":1})).unwrap();assert_eq!(v["state"],expected);assert_eq!(v["checks"][0]["externalWrites"],0);assert_eq!(v["checks"][0]["authenticated"],false);if expected=="UNKNOWN"{assert!(call(&e,json!({"operation":"testConnection","connectionId":c["id"],"revision":3})).is_err());}}
}
#[test]fn provider_no_write_arbitrary_url_or_oauth_actions(){
 let e=Engine::open(&root(),None).unwrap();for op in ["createDraft","schedulePost","publishPost","getPostStatus"]{assert!(call(&e,json!({"operation":op})).is_err())}
 assert!(call(&e,json!({"operation":"connect","providerId":"publer","authMethod":"apiKey","source":{"kind":"mock"}})).is_err());
 assert!(call(&e,json!({"operation":"connect","providerId":"buffer","authMethod":"oauth","source":{"kind":"nativeVault"}})).is_err());
 assert!(call(&e,json!({"operation":"connect","providerId":"buffer","authMethod":"apiKey","source":{"kind":"mock"}})).is_err());
 assert!(call(&e,json!({"operation":"connect","providerId":"buffer","authMethod":"apiKey","source":{"kind":"nativeVault"},"endpoint":"https://evil.invalid"})).is_err());
 assert!(provider_buffer::approved_body(br#"{"query":"mutation { createPost }"}"#)==false);
 assert!(!provider_buffer::approved_body(br#"{"query":"query { account { email } }"}"#));
 assert!(provider_buffer::approved_body(&serde_json::to_vec(&json!({"query":provider_buffer::ACCOUNT})).unwrap()));
 assert!(provider_buffer::approved_body(&serde_json::to_vec(&json!({"query":provider_buffer::channels_query("organization-1").unwrap()})).unwrap()));
 assert!(provider_buffer::channels_query("id\" }) { mutation").is_err());
}
struct Scripted {replies:std::collections::VecDeque<Result<Reply,Error>>,calls:usize}
impl ReadTransport for Scripted{fn query(&mut self,b:&[u8])->Result<Reply,Error>{assert!(provider_buffer::approved_body(b));self.calls+=1;self.replies.pop_front().expect("Unexpected retry")}}
fn response(v:Value)->Result<Reply,Error>{Ok(Reply{status:200,bytes:serde_json::to_vec(&v).unwrap()})}
#[test]fn provider_buffer_maps_only_actual_response_and_propagates_failures(){
 let account=json!({"data":{"account":{"id":"a1","organizations":[{"id":"org1","name":"Returned workspace"}]}}});
 let channels=json!({"data":{"channels":[{"id":"c1","name":"Returned name","service":"returned-platform","isQueuePaused":true}]}});
 let mut t=Scripted{replies:[response(account.clone()),response(channels)].into(),calls:0};let d=provider_buffer::discover(&mut t);assert!(d.error.is_none());assert!(d.authenticated);assert_eq!(d.channels[0].workspace_id,"org1");assert_eq!(d.channels[0].platform,"returned-platform");assert_eq!(t.calls,2);assert_eq!(d.evidence.len(),2);
 for (reply,code) in [(Ok(Reply{status:401,bytes:vec![]}),"AUTHENTICATION_FAILED"),(Ok(Reply{status:403,bytes:vec![]}),"FORBIDDEN"),(Ok(Reply{status:429,bytes:vec![]}),"RATE_LIMITED"),(Ok(Reply{status:302,bytes:vec![]}),"REDIRECT_REJECTED"),(Err(Error::Timeout),"TIMEOUT"),(Err(Error::Transport),"TRANSPORT_UNKNOWN"),(response(json!({"errors":[{"message":"secret should never appear"}],"data":{"account":{"id":"a"}}})),"GRAPHQL_ERROR"),(response(json!({"data":{"account":null}})),"INVALID_RESPONSE")]{let mut t=Scripted{replies:[reply].into(),calls:0};let d=provider_buffer::discover(&mut t);assert_eq!(d.error.unwrap().code,code);assert_eq!(t.calls,1);assert!(d.channels.is_empty());assert!(!serde_json::to_string(&d.evidence).unwrap().contains("secret should never appear"));}
 let mut t=Scripted{replies:[response(account),Err(Error::Timeout)].into(),calls:0};let d=provider_buffer::discover(&mut t);assert!(d.authenticated);assert!(d.channels.is_empty());assert_eq!(d.error.unwrap().code,"TIMEOUT");
}
#[test]fn provider_existing_credential_is_referenced_not_copied_or_exposed(){
 let p=root();let f=p.join(".env");let secret="synthetic-provider-secret-only";fs::write(&f,format!("BUFFER_API_KEY={secret}\nOTHER=not-a-provider-secret\n")).unwrap();let original=fs::read(&f).unwrap();
 let e=Engine::open(&p.join("profile"),None).unwrap();let source=Source::ExistingEnv{path:f.to_string_lossy().into(),field:"BUFFER_API_KEY".into()};
 assert_eq!(crate::provider_credentials::resolve("buffer","ref",&source).unwrap().bytes(),secret.as_bytes());assert!(crate::provider_credentials::resolve("mock","ref",&source).is_err());
 let c=providers::perform(&e,Action::Connect{provider_id:"buffer".into(),auth_method:"apiKey".into(),source,fields:HashMap::new()}).unwrap();
 let view=call(&e,json!({"operation":"get"})).unwrap();assert!(!view.to_string().contains(secret));assert!(!view.to_string().contains(f.to_str().unwrap()));assert_eq!(c["state"],"CONFIGURED");
 let d=e.data.lock().unwrap();for table in ["provider_connections","provider_credential_refs"]{let rows=d.store.0.prepare(&format!("SELECT * FROM {table}")).unwrap().query_map([],|r|{let mut s=String::new();for i in 0..r.as_ref().column_count(){if let Ok(v)=r.get::<_,String>(i){s.push_str(&v)}}Ok(s)}).unwrap().collect::<Result<Vec<_>,_>>().unwrap();assert!(rows.iter().all(|v|!v.contains(secret)));}
 assert_eq!(fs::read(f).unwrap(),original);assert!(Secret::from_bytes(b"bad\r\nkey".to_vec()).is_err());
}
#[test]fn provider_future_publication_approval_binds_all_target_and_text_fields(){
 use sha2::{Digest,Sha256};let text="本文".to_string();let b=PostBinding{job_id:"j".into(),provider_id:"p".into(),connection_id:"c".into(),workspace_id:"w".into(),channel_id:"ch".into(),platform:"s".into(),text_sha256:format!("{:x}",Sha256::digest(text.as_bytes())),text,method:"schedule".into(),scheduled_at:Some("2026-11-01T01:00:00Z".into())};let a=PublicationApproval{approval_id:"publication-only".into(),binding:b.clone()};assert!(a.matches(&b));
 for field in ["jobId","providerId","connectionId","workspaceId","channelId","platform","text","textSha256","method","scheduledAt"]{let mut changed=serde_json::to_value(&b).unwrap();changed[field]=json!("changed");let changed:PostBinding=serde_json::from_value(changed).unwrap();assert!(!a.matches(&changed),"{field}");}
}
