//! Connector Hub v1: dashboard-neutral metadata plus bounded trusted transports.
//! Existing Provider/API rows are projected, never migrated or duplicated.
use crate::{engine::Engine, model::{id,now}, connector_mcp as mcp, connector_webhook as webhook, providers};
use rusqlite::{Connection, params, OptionalExtension};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::sync::Arc;

#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Connector {
    pub id:String, pub display_name:String, pub provider_id:String, pub transport:String,
    pub endpoint_id:String, pub endpoint_ref:Option<String>, pub auth_method:String, pub credential_ref:Option<String>,
    pub capabilities:Value, pub state:String, pub last_checked_at:Option<String>, pub revision:u64,
    pub version:u32, pub reason:String, pub scope:String, pub discovery:mcp::Discovery,
    pub settings:Value, pub attempts:Vec<Value>,
}

pub fn capabilities(read:bool)->Value { json!({"listWorkspaces":read,"listChannels":read,"getCapabilities":true,
    "createDraft":false,"schedulePost":false,"publishPost":false,"getPostStatus":false}) }

#[derive(Deserialize)]
#[serde(tag="operation",rename_all="camelCase",rename_all_fields="camelCase",deny_unknown_fields)]
pub enum Action {
    Get,
    Register { display_name:String, provider_id:String, transport:String, endpoint_id:String, auth_method:String,
        #[serde(default)] settings:Value },
    Check { connector_id:String, revision:u64 },
    Disconnect { connector_id:String, revision:u64 },
    GetCapabilities { connector_id:String },
    ListWorkspaces { connector_id:String },
    ListChannels { connector_id:String, workspace_id:String },
    SendMock { connector_id:String, revision:u64 },
}

fn init(db:&Connection)->Result<(),String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS hub_connectors(id TEXT PRIMARY KEY,revision INTEGER NOT NULL,json TEXT NOT NULL);")
        .map_err(|_|"Connector設定領域を利用できません".into())
}
fn load(db:&Connection,id:&str)->Result<Connector,String> {
    let text:String=db.query_row("SELECT json FROM hub_connectors WHERE id=?1",[id],|r|r.get(0)).optional()
        .map_err(|_|"Connector読取失敗")?.ok_or("Connectorがありません")?;
    serde_json::from_str(&text).map_err(|_|"Connector形式エラー".into())
}
fn current(db:&Connection,id:&str,revision:u64)->Result<Connector,String> {
    let c=load(db,id)?;
    if c.revision!=revision { return Err("設定が更新されています。再読込してください".into()); } Ok(c)
}
fn put(db:&Connection,c:&mut Connector)->Result<(),String> {
    let old=c.revision;c.revision+=1;
    if db.execute("UPDATE hub_connectors SET revision=?1,json=?2 WHERE id=?3 AND revision=?4",params![c.revision,serde_json::to_string(c).unwrap(),c.id,old]).map_err(|_|"Connector保存失敗")? != 1 {
        return Err("設定revisionが競合しました".into());
    } Ok(())
}
fn api_state(e:&Arc<Engine>)->Result<Value,String> { providers::perform(e,providers::Action::Get) }
fn api<'a>(state:&'a Value,id:&str)->Result<&'a Value,String> {
    let id=id.strip_prefix("api:").ok_or("API識別子不正")?;
    state["connections"].as_array().unwrap().iter().find(|c|c["id"]==id).ok_or("API接続がありません".into())
}
fn project(c:&Value)->Value {
    let mock=c["providerId"]=="mock";
    let state=match c["state"].as_str().unwrap_or("UNKNOWN") {
        "TARGETS_VERIFIED"|"MOCK_VERIFIED"=>"READ_VERIFIED", "AUTHENTICATED"=>"CONNECTED",
        "CONFIGURED"=>"CONFIGURED", "DISCONNECTED"=>"DISCONNECTED", "CHECKING_UNKNOWN"|"UNKNOWN"=>"UNKNOWN",
        "AUTHENTICATION_FAILED"|"CREDENTIAL_UNAVAILABLE"|"FORBIDDEN"=>"AUTH_REQUIRED",_=>"FAILED",
    };
    let mut caps=capabilities(state=="READ_VERIFIED");
    if c["providerId"]=="buffer"{caps["schedulePost"]=json!(state=="READ_VERIFIED");caps["publishPost"]=json!(state=="READ_VERIFIED");caps["getPostStatus"]=json!(state=="READ_VERIFIED");}
    json!({"id":format!("api:{}",c["id"].as_str().unwrap()),"displayName":format!("{} · 既存API接続",c["providerId"].as_str().unwrap()),
        "providerId":c["providerId"],"transport":"api","endpointId":if mock {"offline:mock-provider"} else {"https://api.buffer.com/"},
        "endpointRef":null,"authMethod":c["authMethod"],"credentialRef":c["credentialRef"],"capabilities":caps,"state":state,
        "lastCheckedAt":c["checks"].as_array().and_then(|a|a.last()).map(|c|&c["endedAt"]),"revision":c["revision"],"version":1,
        "reason":"既存Provider設定・取得結果の共通表示。Threadsの予約・即投稿は完成品の投稿画面で個別に承認します。再取得は明示操作のみ。",
        "scope":if mock {"OFFLINE_SIMULATION"} else {"EXISTING_API_EVIDENCE"},"discovery":mcp::Discovery::default(),
        "settings":{"apiStyle":if mock {"Offline"} else {"GraphQL"}},"attempts":c["checks"]})
}

pub fn perform(e:&Arc<Engine>,action:Action)->Result<Value,String> {
    // API actions retain the original Provider credential domain and CAS.
    match &action {
        Action::Check{connector_id,revision} if connector_id.starts_with("api:") => {
            return providers::perform(e,providers::Action::TestConnection{connection_id:connector_id[4..].into(),revision:*revision}).map(|c|project(&c));
        },
        Action::Disconnect{connector_id,revision} if connector_id.starts_with("api:") => {
            return providers::perform(e,providers::Action::Disconnect{connection_id:connector_id[4..].into(),revision:*revision}).map(|c|project(&c));
        },_=>()
    }
    let prior=api_state(e)?;
    let d=e.data.lock().map_err(|_|"Core lock")?;let db=&d.store.0;init(db)?;
    match action {
        Action::Get=>{
            let mut list=prior["connections"].as_array().unwrap().iter().map(project).collect::<Vec<_>>();
            let mut q=db.prepare("SELECT json FROM hub_connectors ORDER BY rowid").map_err(|_|"Connector読取失敗")?;
            for row in q.query_map([],|r|r.get::<_,String>(0)).map_err(|_|"Connector読取失敗")? {
                let c:Connector=serde_json::from_str(&row.map_err(|_|"Connector読取失敗")?).map_err(|_|"Connector形式エラー")?;
                list.push(json!(c));
            }
            Ok(json!({"version":1,"transports":["api","mcp","webhook"],"connectors":list,"externalWrites":0,"publication":"NOT_PERFORMED"}))
        },
        Action::Register{display_name,provider_id,transport,endpoint_id,auth_method,settings}=>{
            if display_name.trim().is_empty() || display_name.len()>120 { return Err("接続名を入力してください（120文字以内）".into()); }
            let mut state="CONFIGURED";let reason;
            match transport.as_str() {
                "api"=>return Err("既存API Adapterの設定画面で登録してください。任意HTTP設定は利用できません".into()),
                "mcp"=>{
                    if !settings.is_null() && settings!=json!({}) { return Err("未対応のMCP設定・資格情報です".into()); }
                    match provider_id.as_str() {
                        "buffer" if endpoint_id==mcp::BUFFER_ENDPOINT && ["oauth","apiKey"].contains(&auth_method.as_str())=>{
                            state="AUTH_REQUIRED";reason="MCP用の明示認証が必要です。APIの資格情報は転用しません。OAuth・MCP資格情報登録は未実装。認証なしの仕様確認のみ可能";
                        },
                        "mock-mcp" if endpoint_id=="offline:mock-mcp" && auth_method=="simulation"=>reason="ローカル模擬MCP。実サービス接続ではありません",
                        _=>return Err("信頼済みMCP定義が必要です。今回登録できるのはBuffer公式とMockのみです".into()),
                    }
                },
                "webhook"=>{
                    let mock=provider_id=="mock-webhook";
                    if !mock && provider_id!="make" { return Err("許可済みホスト定義が必要です。n8n等の追加は未実装".into()); }
                    webhook::validate_url(&endpoint_id,mock)?;
                    if !["none","bearer"].contains(&auth_method.as_str()) && !(mock && auth_method=="syntheticBearer") { return Err("未対応のWebhook認証方式".into()); }
                    if mock && auth_method=="bearer" { return Err("Mockに実資格情報は渡せません".into()); }
                    let obj=settings.as_object().ok_or("Webhook設定が必要です")?;
                    if obj.keys().any(|k|!["method","schema","timeoutMs","scenario"].contains(&k.as_str()))
                        || settings["method"]!="POST" || settings["schema"]!=webhook::SCHEMA
                        || settings["timeoutMs"].as_u64().is_none_or(|n|!(100..=5000).contains(&n))
                        || !["accepted","httpError","timeout","unknown"].contains(&settings["scenario"].as_str().unwrap_or("")) { return Err("固定送信schema・POST・タイムアウト設定を確認してください".into()); }
                    if mock { reason="固定ループバックMock専用。受付はSNS公開を意味しません"; }
                    else { state="UNSUPPORTED";reason="設定保存のみ。本番送信・認証登録・DNS/接続先IP固定は未実装。外部URLへ送信しません"; }
                },
                _=>return Err("未対応の接続方式".into()),
            }
            let cid=id();let mut endpoint_ref=None;let mut public_endpoint=endpoint_id.clone();
            if transport=="webhook" && provider_id!="mock-webhook" {
                let reference=format!("ai-guild/connector/webhook/{cid}/endpoint");
                crate::provider_credentials::protect_connector_endpoint(&reference,&endpoint_id)?;
                public_endpoint=format!("https://{}/[protected]",endpoint_id.strip_prefix("https://").unwrap().split('/').next().unwrap());
                endpoint_ref=Some(reference);
            }
            let c=Connector{id:cid,display_name:display_name.trim().into(),provider_id,transport,endpoint_id:public_endpoint,endpoint_ref,auth_method,credential_ref:None,
                capabilities:capabilities(false),state:state.into(),last_checked_at:None,revision:1,version:1,reason:reason.into(),
                scope:if state=="AUTH_REQUIRED" {"REAL_MCP_UNAUTHENTICATED"} else if state=="UNSUPPORTED" {"CONFIGURATION_ONLY"} else {"OFFLINE_SIMULATION"}.into(),
                discovery:mcp::Discovery::default(),settings,attempts:vec![]};
            db.execute("INSERT INTO hub_connectors VALUES(?1,?2,?3)",params![c.id,c.revision,serde_json::to_string(&c).unwrap()]).map_err(|_|"Connector保存失敗")?;
            Ok(json!(c))
        },
        Action::Check{connector_id,revision}=>{
            let mut c=current(db,&connector_id,revision)?;
            if c.transport!="mcp" { return Err("Webhookへの送信は接続確認ではありません。API以外の接続確認はMCPのみ対応".into()); }
            if ["UNKNOWN","DISCONNECTED"].contains(&c.state.as_str()) || !c.attempts.is_empty() { return Err("既存の確認記録を保持して停止します。自動再送しません".into()); }
            let attempt=id();c.state="UNKNOWN".into();c.last_checked_at=Some(now());
            c.attempts.push(json!({"id":attempt,"startedAt":now(),"endedAt":null,"status":"UNKNOWN","scope":c.scope,"externalWrites":0}));put(db,&mut c)?;
            let result=if c.provider_id=="mock-mcp" {mcp::discover(&mut mcp::Mock,&c.provider_id)} else {mcp::discover(&mut mcp::Buffer,&c.provider_id)};
            match result {
                Ok(discovery)=>{c.discovery=discovery;c.state="CONNECTED".into();c.reason="能力とツール一覧を取得。業務データ取得と投稿は未検証・実行不可".into();},
                Err(code)=>{c.state=match code.as_str(){"AUTH_REQUIRED"=>"AUTH_REQUIRED","TIMEOUT"|"TRANSPORT_UNKNOWN"=>"UNKNOWN",s if s.contains("UNSUPPORTED")=>"UNSUPPORTED",_=>"FAILED"}.into();c.reason=code;},
            }
            c.last_checked_at=Some(now());let a=c.attempts.last_mut().unwrap();a["endedAt"]=json!(now());a["status"]=json!(c.state);a["result"]=json!(c.reason);
            put(db,&mut c)?;Ok(json!(c))
        },
        Action::SendMock{connector_id,revision}=>{
            let mut c=current(db,&connector_id,revision)?;
            if c.transport!="webhook" || c.provider_id!="mock-webhook" || c.state!="CONFIGURED" || !c.attempts.is_empty() { return Err("Mock専用の未送信設定だけを一度使用できます。本番・結果不明・送信済みは送信禁止".into()); }
            webhook::validate_url(&c.endpoint_id,true)?;
            let attempt=id();let body=webhook::payload(&c.id,&attempt);
            use sha2::{Digest,Sha256};let hash=crate::hashing::hex(&Sha256::digest(serde_json::to_vec(&body).unwrap()));
            c.state="UNKNOWN".into();c.last_checked_at=Some(now());
            c.attempts.push(json!({"id":attempt,"startedAt":now(),"endedAt":null,"status":"UNKNOWN","payloadSha256":hash,"externalWrites":0,"scope":"LOCAL_MOCK_ONLY"}));put(db,&mut c)?;
            let result=webhook::send_mock(&body,&c.auth_method,c.settings["scenario"].as_str().unwrap(),c.settings["timeoutMs"].as_u64().unwrap());
            let receipt=match result {Ok(v)=>v,Err(code)=>json!({"receipt":"UNKNOWN","error":code})};
            c.state=match receipt["receipt"].as_str(){Some("ACCEPTED")=>"CONNECTED",Some("FAILED")=>"FAILED",_=>"UNKNOWN"}.into();
            c.reason=match c.state.as_str(){"CONNECTED"=>"ローカルMock受付済み。外部処理・SNS公開は未検証","FAILED"=>"Mock HTTPエラー。記録を維持し再送停止",_=>"受付結果不明。無条件に再試行しません"}.into();
            c.last_checked_at=Some(now());let a=c.attempts.last_mut().unwrap();a["endedAt"]=json!(now());a["status"]=json!(c.state);a["receipt"]=receipt;
            put(db,&mut c)?;Ok(json!(c))
        },
        Action::Disconnect{connector_id,revision}=>{let mut c=current(db,&connector_id,revision)?;c.state="DISCONNECTED".into();c.reason="Coreでの接続利用を停止しました。サービスの認証は変更していません".into();put(db,&mut c)?;Ok(json!(c))},
        Action::GetCapabilities{connector_id}=>{
            if connector_id.starts_with("api:") {Ok(project(api(&prior,&connector_id)?)["capabilities"].clone())}
            else {Ok(load(db,&connector_id)?.capabilities)}
        },
        Action::ListWorkspaces{connector_id}=>{
            if connector_id.starts_with("api:") {Ok(api(&prior,&connector_id)?["workspaces"].clone())}
            else {Err("UNSUPPORTED: このConnectorの業務データ取得は未実装です".into())}
        },
        Action::ListChannels{connector_id,workspace_id}=>{
            if connector_id.starts_with("api:") {Ok(json!(api(&prior,&connector_id)?["channels"].as_array().unwrap().iter().filter(|c|c["workspaceId"]==workspace_id).collect::<Vec<_>>()))}
            else {Err("UNSUPPORTED: 受付やツール発見を投稿先取得へ読み替えません".into())}
        },
    }
}

// Additive future approval contract; no executor. Reuse existing full post binding.
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ExternalApproval {pub connector_id:String,pub transport:String,pub approval:providers::PublicationApproval}
