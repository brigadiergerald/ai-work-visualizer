use crate::{core_api,engine::Engine,connector_mcp as mcp,connector_webhook as webhook};
use serde_json::{json,Value};
use std::{collections::VecDeque,fs,path::PathBuf,sync::Arc};
fn root()->PathBuf{let p=std::env::var_os("AI_GUILD_TEST_ROOT").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("target/test-data/connectors")).join(crate::model::id());fs::create_dir_all(&p).unwrap();p}
fn call(e:&Arc<Engine>,request:Value)->Result<Value,String>{core_api::perform(e,serde_json::from_value(json!({"action":"connectors","request":request})).map_err(|e|e.to_string())?)}
fn register(e:&Arc<Engine>,transport:&str,provider:&str,endpoint:&str,auth:&str,settings:Value)->Result<Value,String>{call(e,json!({"operation":"register","displayName":"テスト接続","transport":transport,"providerId":provider,"endpointId":endpoint,"authMethod":auth,"settings":settings}))}
fn settings(scenario:&str)->Value{json!({"method":"POST","schema":webhook::SCHEMA,"timeoutMs":150,"scenario":scenario})}

#[test]fn connector_contract_existing_api_projection_common_reads_and_preservation(){
    let p=root();let e=Engine::open(&p,None).unwrap();let before=serde_json::to_value(e.snapshot()).unwrap();
    let c=crate::providers::perform(&e,serde_json::from_value(json!({"operation":"connect","providerId":"mock","authMethod":"simulation","source":{"kind":"mock"},"fields":{"scenario":"success"}})).unwrap()).unwrap();
    let id=c["id"].as_str().unwrap();
    crate::providers::perform(&e,serde_json::from_value(json!({"operation":"testConnection","connectionId":id,"revision":1})).unwrap()).unwrap();
    let prior=crate::providers::perform(&e,crate::providers::Action::Get).unwrap();let cid=format!("api:{id}");
    let hub=call(&e,json!({"operation":"get"})).unwrap();assert_eq!(hub["transports"],json!(["api","mcp","webhook"]));
    assert_eq!(hub["connectors"][0]["state"],"READ_VERIFIED");assert_eq!(hub["connectors"][0]["scope"],"OFFLINE_SIMULATION");
    assert_eq!(call(&e,json!({"operation":"listWorkspaces","connectorId":cid})).unwrap(),prior["connections"][0]["workspaces"]);
    assert_eq!(call(&e,json!({"operation":"listChannels","connectorId":cid,"workspaceId":"mock-workspace"})).unwrap(),prior["connections"][0]["channels"]);
    assert_eq!(call(&e,json!({"operation":"getCapabilities","connectorId":cid})).unwrap()["publishPost"],false);
    let m=register(&e,"mcp","buffer",mcp::BUFFER_ENDPOINT,"oauth",json!({})).unwrap();assert_eq!(m["state"],"AUTH_REQUIRED");assert!(m["credentialRef"].is_null());
    assert_eq!(crate::providers::perform(&e,crate::providers::Action::Get).unwrap(),prior);
    assert_eq!(serde_json::to_value(e.snapshot()).unwrap(),before);
    drop(e);let e=Engine::open(&p,None).unwrap();assert_eq!(call(&e,json!({"operation":"get"})).unwrap()["connectors"].as_array().unwrap().len(),2);
}

#[test]fn connector_mcp_discovery_schema_classification_and_no_tools_executor(){
    let p=root();let e=Engine::open(&p,None).unwrap();let c=register(&e,"mcp","mock-mcp","offline:mock-mcp","simulation",json!({})).unwrap();
    let c=call(&e,json!({"operation":"check","connectorId":c["id"],"revision":1})).unwrap();
    assert_eq!(c["state"],"CONNECTED");assert_eq!(c["discovery"]["protocolVersion"],mcp::VERSION);
    let tools=c["discovery"]["tools"].as_array().unwrap();assert_eq!(tools.len(),5);
    assert_eq!(tools[0]["category"],"READ");assert_eq!(tools[0]["mappedCapability"],"listWorkspaces");
    assert_eq!(tools[2]["category"],"PUBLICATION");assert_eq!(tools[3]["category"],"DELETE");assert_eq!(tools[4]["category"],"UNKNOWN");
    assert!(tools.iter().all(|t|t["executionAllowed"]==false));assert_eq!(c["capabilities"]["listChannels"],false);
    assert!(call(&e,json!({"operation":"listChannels","connectorId":c["id"],"workspaceId":"any"})).unwrap_err().contains("UNSUPPORTED"));
    assert!(call(&e,json!({"operation":"check","connectorId":c["id"],"revision":3})).is_err());
    for op in ["callTool","tools/call","createDraft","publishPost","schedulePost","deletePost","executeQuery","executeMutation"] {
        assert!(call(&e,json!({"operation":op,"connectorId":c["id"]})).is_err());
    }
    assert!(mcp::parse_tools("buffer",&json!({"tools":[{"name":"x","inputSchema":{"type":"string"}}]})).is_err());
    assert!(mcp::parse_tools("buffer",&json!({"tools":[{"name":"x","inputSchema":{"type":"object","required":["missing"]}}]})).is_err());
    let t=json!({"name":"get_account","inputSchema":{"type":"object"},"annotations":{"readOnlyHint":true}});
    assert_eq!(mcp::parse_tools("untrusted",&json!({"tools":[t]})).unwrap()[0].category,"UNKNOWN");
    drop(e);let e=Engine::open(&p,None).unwrap();assert_eq!(call(&e,json!({"operation":"get"})).unwrap()["connectors"][0],c);
    let disconnected=call(&e,json!({"operation":"disconnect","connectorId":c["id"],"revision":3})).unwrap();assert_eq!(disconnected["state"],"DISCONNECTED");
}

struct Script {queue:VecDeque<Result<mcp::Reply,String>>,sent:Vec<Value>}
impl mcp::Transport for Script{fn post(&mut self,v:Value)->Result<mcp::Reply,String>{self.sent.push(v);self.queue.pop_front().expect("unexpected retry")}}
fn rpc(id:u64,result:Value)->Result<mcp::Reply,String>{Ok(mcp::Reply{status:200,body:json!({"jsonrpc":"2.0","id":id,"result":result})})}
fn init()->Value{json!({"protocolVersion":mcp::VERSION,"serverInfo":{"name":"mock","version":"1"},"capabilities":{"tools":{}}})}

#[test]fn connector_mcp_auth_timeout_disconnect_version_ids_and_pagination(){
    for (reply,expected) in [(Ok(mcp::Reply{status:401,body:Value::Null}),"AUTH_REQUIRED"),(Ok(mcp::Reply{status:302,body:Value::Null}),"REDIRECT_REJECTED"),(Err("TIMEOUT".into()),"TIMEOUT"),(Err("TRANSPORT_UNKNOWN".into()),"TRANSPORT_UNKNOWN"),(rpc(99,init()),"INVALID_RPC_RESPONSE"),(rpc(1,json!({"protocolVersion":"unknown"})),"UNSUPPORTED_PROTOCOL")] {
        let mut t=Script{queue:[reply].into(),sent:vec![]};assert_eq!(mcp::discover(&mut t,"buffer").err().unwrap(),expected);assert_eq!(t.sent.len(),1);
    }
    let mut t=Script{queue:[rpc(1,init()),Ok(mcp::Reply{status:202,body:Value::Null}),rpc(2,json!({"tools":[],"nextCursor":"next"})),rpc(3,json!({"tools":[{"name":"execute_query","inputSchema":{"type":"object"}}]}))].into(),sent:vec![]};
    let d=mcp::discover(&mut t,"buffer").unwrap();assert_eq!(d.tools[0].category,"UNKNOWN");assert_eq!(t.sent[3]["params"]["cursor"],"next");
    assert!(t.sent.iter().all(|m|["initialize","notifications/initialized","tools/list"].contains(&m["method"].as_str().unwrap())));
    assert!(!serde_json::to_string(&d).unwrap().contains("instructions"));
}

#[test]fn connector_webhook_local_http_accept_error_timeout_unknown_dedup_restart(){
    let p=root();let e=Engine::open(&p,None).unwrap();
    for (scenario,auth,expected) in [("accepted","none","CONNECTED"),("accepted","syntheticBearer","CONNECTED"),("httpError","none","FAILED"),("timeout","none","UNKNOWN"),("unknown","none","UNKNOWN")] {
        let c=register(&e,"webhook","mock-webhook",webhook::MOCK_URL,auth,settings(scenario)).unwrap();
        assert!(call(&e,json!({"operation":"check","connectorId":c["id"],"revision":1})).is_err());
        let c=call(&e,json!({"operation":"sendMock","connectorId":c["id"],"revision":1})).unwrap();assert_eq!(c["state"],expected,"{scenario}");
        assert_eq!(c["attempts"].as_array().unwrap().len(),1);assert_eq!(c["attempts"][0]["externalWrites"],0);
        assert!(call(&e,json!({"operation":"sendMock","connectorId":c["id"],"revision":3})).is_err());
        assert_eq!(c["capabilities"]["publishPost"],false);
    }
    let before=call(&e,json!({"operation":"get"})).unwrap();drop(e);let e=Engine::open(&p,None).unwrap();assert_eq!(call(&e,json!({"operation":"get"})).unwrap(),before);
    let prod=register(&e,"webhook","make","https://hook.eu1.make.com/configuration-only","none",settings("accepted")).unwrap();assert_eq!(prod["state"],"UNSUPPORTED");
    assert!(call(&e,json!({"operation":"sendMock","connectorId":prod["id"],"revision":1})).is_err());
    assert_eq!(prod["endpointId"],"https://hook.eu1.make.com/[protected]");
    assert!(prod["endpointRef"].as_str().unwrap().starts_with("ai-guild/connector/webhook/"));
    {let d=e.data.lock().unwrap();let saved:String=d.store.0.query_row("SELECT json FROM hub_connectors WHERE id=?1",[prod["id"].as_str().unwrap()],|r|r.get(0)).unwrap();assert!(!saved.contains("configuration-only"));}
    #[cfg(windows)]unsafe{
        let target:Vec<u16>=prod["endpointRef"].as_str().unwrap().encode_utf16().chain(Some(0)).collect();
        assert_ne!(windows_sys::Win32::Security::Credentials::CredDeleteW(target.as_ptr(),windows_sys::Win32::Security::Credentials::CRED_TYPE_GENERIC,0),0);
    }
}

#[test]fn connector_credentials_urls_and_untrusted_configuration_fail_closed(){
    let e=Engine::open(&root(),None).unwrap();
    for url in ["http://hook.eu1.make.com/a","https://127.0.0.1/a","https://169.254.169.254/a","https://[::1]/a","https://localhost/a","https://10.0.0.1/a","https://hook.eu1.make.com.evil/a","https://hook.eu1.make.com:443/a","https://hook.eu1.make.com@evil/a","https://hook.eu1.make.com/a?secret=x","https://hook.eu1.make.com/%2e%2e","https://evil/a"] {
        assert!(webhook::validate_url(url,false).is_err(),"{url}");
    }
    assert!(webhook::validate_url(webhook::MOCK_URL,false).is_err());assert!(webhook::validate_url("http://localhost:18791/connector-hub/mock",true).is_err());
    assert!(register(&e,"mcp","buffer","https://evil/mcp","apiKey",json!({})).is_err());
    assert!(register(&e,"mcp","buffer",mcp::BUFFER_ENDPOINT,"apiKey",json!({"credentialRef":"api:stolen"})).is_err());
    assert!(register(&e,"webhook","mock-webhook",webhook::MOCK_URL,"bearer",settings("accepted")).is_err());
    assert!(register(&e,"api","buffer","https://evil","apiKey",json!({})).is_err());
    assert!(call(&e,json!({"operation":"register","displayName":"x","transport":"mcp","providerId":"buffer","endpointId":mcp::BUFFER_ENDPOINT,"authMethod":"apiKey","credentialRef":"anything"})).is_err());
    let source=crate::provider_credentials::Source::NativeVault;
    assert!(crate::provider_credentials::validate_source("mcp-buffer",&source).is_err());
}

#[test]fn connector_crash_intent_unknown_and_stale_revision_never_resend(){
    let p=root();let e=Engine::open(&p,None).unwrap();let mut c=register(&e,"webhook","mock-webhook",webhook::MOCK_URL,"none",settings("accepted")).unwrap();
    c["state"]=json!("UNKNOWN");c["revision"]=json!(2);c["attempts"]=json!([{"id":"durable-intent","status":"UNKNOWN","startedAt":"2026-10-28T00:00:00Z","endedAt":null}]);
    {let d=e.data.lock().unwrap();d.store.0.execute("UPDATE hub_connectors SET revision=2,json=?1 WHERE id=?2",rusqlite::params![c.to_string(),c["id"].as_str().unwrap()]).unwrap();}
    drop(e);let e=Engine::open(&p,None).unwrap();
    assert!(call(&e,json!({"operation":"sendMock","connectorId":c["id"],"revision":1})).is_err());assert!(call(&e,json!({"operation":"sendMock","connectorId":c["id"],"revision":2})).is_err());
    assert_eq!(call(&e,json!({"operation":"get"})).unwrap()["connectors"][0],c);
}
