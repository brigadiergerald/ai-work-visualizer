//! Bounded MCP 2025-11-25 client. Remote text is data, never host instructions.
//! No tools/call, resource execution, sampling, roots or credential import surface.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

pub const VERSION: &str = "2025-11-25";
pub const BUFFER_ENDPOINT: &str = "https://mcp.buffer.com/mcp";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub category: String,
    pub mapped_capability: Option<String>,
    pub execution_allowed: bool,
    pub reason: String,
}

// Classification comes from a host-owned service registry, never annotations/names alone.
fn category(provider: &str, name: &str) -> &'static str {
    if provider != "buffer" && provider != "mock-mcp" { return "UNKNOWN"; }
    match name {
        "get_account" | "list_channels" => "READ",
        "create_post" | "edit_post" => "PUBLICATION",
        "delete_post" => "DELETE",
        "execute_mutation" => "UPDATE",
        // Generic queries may access arbitrary data; no free-form query execution.
        "execute_query" => "UNKNOWN",
        _ => "UNKNOWN",
    }
}

pub fn parse_tools(provider: &str, value: &Value) -> Result<Vec<Tool>, String> {
    let tools = value["tools"].as_array().ok_or("INVALID_TOOLS")?;
    if tools.len() > 256 { return Err("TOO_MANY_TOOLS".into()); }
    let mut names = HashSet::new();
    tools.iter().map(|t| {
        let name = t["name"].as_str().filter(|s| !s.is_empty() && s.len() <= 128).ok_or("INVALID_TOOL_NAME")?;
        if !names.insert(name) { return Err("DUPLICATE_TOOL".into()); }
        let schema = &t["inputSchema"];
        if schema["type"] != "object" || !schema.is_object() || schema.to_string().len() > 65536
            || schema.get("properties").is_some_and(|p| !p.is_object())
            || schema.get("required").is_some_and(|r| r.as_array().is_none_or(|a| a.iter().any(|v| v.as_str().is_none_or(|s| schema["properties"].get(s).is_none())))) {
            return Err("INVALID_INPUT_SCHEMA".into());
        }
        let description = t.get("description").and_then(Value::as_str).unwrap_or("");
        if description.len() > 8192 { return Err("TOOL_DESCRIPTION_TOO_LARGE".into()); }
        let kind = category(provider, name);
        let mapped = match (kind, name) {
            ("READ", "get_account") => Some("listWorkspaces".to_string()),
            ("READ", "list_channels") => Some("listChannels".to_string()),
            _ => None,
        };
        Ok(Tool { name: name.into(), description: description.into(), input_schema: schema.clone(),
            category: kind.into(), mapped_capability: mapped, execution_allowed: false,
            reason: if kind == "READ" { "読み取り候補。取得schema・応答マッピングの検証と個別実装が必要。今回は実行不可" }
                else { "未承認・未知または外部変更を伴うため実行禁止" }.into() })
    }).collect()
}

pub struct Reply { pub status: u32, pub body: Value }
pub trait Transport {
    fn post(&mut self, message: Value) -> Result<Reply, String>;
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub protocol_version: Option<String>,
    pub server_info: Value,
    pub server_capabilities: Value,
    pub tools: Vec<Tool>,
}

fn result(reply: Reply, id: u64) -> Result<Value, String> {
    match reply.status {
        401 | 403 => return Err("AUTH_REQUIRED".into()),
        300..=399 => return Err("REDIRECT_REJECTED".into()),
        429 => return Err("RATE_LIMITED".into()),
        200 => (),
        _ => return Err("MCP_HTTP_ERROR".into()),
    }
    let b = reply.body;
    if b["jsonrpc"] != "2.0" || b["id"] != id || b.get("error").is_some() || !b["result"].is_object() {
        return Err("INVALID_RPC_RESPONSE".into());
    }
    Ok(b["result"].clone())
}

pub fn discover(t: &mut dyn Transport, provider: &str) -> Result<Discovery, String> {
    let init = result(t.post(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":VERSION,"capabilities":{},"clientInfo":{"name":"AI GUILD Connector Hub","version":"1.0.0"}}}))?, 1)?;
    if init["protocolVersion"] != VERSION { return Err("UNSUPPORTED_PROTOCOL".into()); }
    if !init["serverInfo"]["name"].is_string() || !init["serverInfo"]["version"].is_string() || !init["capabilities"].is_object() {
        return Err("INVALID_SERVER_INFO".into());
    }
    // Deliberately exclude server-provided instructions and all unexpected fields.
    let mut d = Discovery { protocol_version: Some(VERSION.into()),
        server_info: json!({"name":init["serverInfo"]["name"],"version":init["serverInfo"]["version"]}),
        server_capabilities: init["capabilities"].clone(), tools: vec![] };
    let notified = t.post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))?;
    if notified.status != 202 && notified.status != 200 { return Err("INITIALIZED_REJECTED".into()); }
    if init["capabilities"].get("tools").is_none() { return Ok(d); }
    let mut cursor: Option<String> = None;
    let mut cursors = HashSet::new();
    for page in 0..8 {
        let id = 2 + page;
        let params = cursor.as_ref().map(|c| json!({"cursor":c})).unwrap_or(json!({}));
        let value = result(t.post(json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":params}))?, id)?;
        let tools = parse_tools(provider, &value)?;
        for tool in tools {
            if d.tools.iter().any(|v| v.name == tool.name) { return Err("DUPLICATE_TOOL".into()); }
            d.tools.push(tool);
        }
        if d.tools.len() > 256 { return Err("TOO_MANY_TOOLS".into()); }
        match value.get("nextCursor") {
            None | Some(Value::Null) => return Ok(d),
            Some(Value::String(c)) if !c.is_empty() && c.len() <= 1024 && cursors.insert(c.clone()) => cursor = Some(c.clone()),
            _ => return Err("INVALID_CURSOR".into()),
        }
    }
    Err("PAGINATION_LIMIT".into())
}

pub struct Mock;
impl Transport for Mock {
    fn post(&mut self, m: Value) -> Result<Reply, String> {
        let result = match m["method"].as_str() {
            Some("initialize") => json!({"protocolVersion":VERSION,"serverInfo":{"name":"Local mock MCP","version":"1"},"capabilities":{"tools":{}},"instructions":"untrusted; ignored"}),
            Some("notifications/initialized") => return Ok(Reply { status:202, body:Value::Null }),
            Some("tools/list") => json!({"tools":[
                {"name":"get_account","description":"模擬アカウント情報","inputSchema":{"type":"object","properties":{}}},
                {"name":"list_channels","description":"模擬チャンネル一覧","inputSchema":{"type":"object","properties":{"organizationId":{"type":"string"}},"required":["organizationId"]}},
                {"name":"create_post","description":"投稿（呼出禁止）","inputSchema":{"type":"object"}},
                {"name":"delete_post","description":"削除（呼出禁止）","inputSchema":{"type":"object"}},
                {"name":"innocent_name","description":"安全という主張は信用しない","inputSchema":{"type":"object"},"annotations":{"readOnlyHint":true}}
            ]}),
            _ => return Err("TOOL_EXECUTION_FORBIDDEN".into()),
        };
        Ok(Reply { status:200, body:json!({"jsonrpc":"2.0","id":m["id"],"result":result}) })
    }
}

// Official fixed endpoint only, unauthenticated discovery. Separate MCP credential
// enrollment/OAuth is intentionally unavailable; no existing API secret is resolved.
pub struct Buffer;
impl Transport for Buffer {
    fn post(&mut self, m: Value) -> Result<Reply, String> { crate::connector_http::buffer_mcp(m) }
}
