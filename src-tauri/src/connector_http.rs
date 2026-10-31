//! Private fixed-host MCP transport, not a user-configurable HTTP client.
use serde_json::Value;
use crate::connector_mcp::{Reply, VERSION};

#[cfg(windows)]
pub fn buffer_mcp(message: Value) -> Result<Reply, String> {
    use windows_sys::Win32::{Networking::WinHttp::*, Foundation::GetLastError};
    if !["initialize", "notifications/initialized", "tools/list"].contains(&message["method"].as_str().unwrap_or("")) {
        return Err("TOOL_EXECUTION_FORBIDDEN".into());
    }
    // Session-bearing/authenticated MCP will require a dedicated reviewed adapter.
    // This v1 unauthenticated probe stops at auth or stateful-server requirements.
    struct Handle(*mut std::ffi::c_void);
    impl Drop for Handle { fn drop(&mut self) { unsafe { WinHttpCloseHandle(self.0); } } }
    fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }
    fn fail() -> String { if unsafe { GetLastError() } == 12002 { "TIMEOUT" } else { "TRANSPORT_UNKNOWN" }.into() }
    unsafe {
        let session = Handle(WinHttpOpen(wide("AI-GUILD-Core/0.1 MCP ReadOnly").as_ptr(), WINHTTP_ACCESS_TYPE_NO_PROXY, std::ptr::null(), std::ptr::null(), 0));
        if session.0.is_null() { return Err(fail()); }
        if WinHttpSetTimeouts(session.0,45000,45000,45000,45000)==0 { return Err(fail()); }
        let attempts=1u32;
        if WinHttpSetOption(session.0,WINHTTP_OPTION_CONNECT_RETRIES,&attempts as *const _ as _,4)==0 { return Err(fail()); }
        let retries=WINHTTP_FAILED_CONNECTION_RETRIES{dwMaxRetries:0,dwAllowedRetryConditions:0};
        if WinHttpSetOption(session.0,WINHTTP_OPTION_FAILED_CONNECTION_RETRIES,&retries as *const _ as _,std::mem::size_of_val(&retries) as u32)==0 { return Err(fail()); }
        let conn=Handle(WinHttpConnect(session.0,wide("mcp.buffer.com").as_ptr(),443,0));
        if conn.0.is_null() { return Err(fail()); }
        let req=Handle(WinHttpOpenRequest(conn.0,wide("POST").as_ptr(),wide("/mcp").as_ptr(),std::ptr::null(),std::ptr::null(),std::ptr::null(),WINHTTP_FLAG_SECURE));
        if req.0.is_null() { return Err(fail()); }
        let policy=WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
        let disabled=WINHTTP_DISABLE_COOKIES|WINHTTP_DISABLE_AUTHENTICATION|WINHTTP_DISABLE_KEEP_ALIVE;
        if WinHttpSetOption(req.0,WINHTTP_OPTION_REDIRECT_POLICY,&policy as *const _ as _,4)==0 || WinHttpSetOption(req.0,WINHTTP_OPTION_DISABLE_FEATURE,&disabled as *const _ as _,4)==0 { return Err(fail()); }
        let headers=wide(&format!("Content-Type: application/json\r\nAccept: application/json, text/event-stream\r\nMCP-Protocol-Version: {VERSION}\r\n"));
        let body=serde_json::to_vec(&message).map_err(|_|"INVALID_REQUEST")?;
        if WinHttpSendRequest(req.0,headers.as_ptr(),(headers.len()-1) as u32,body.as_ptr() as _,body.len() as u32,body.len() as u32,0)==0 || WinHttpReceiveResponse(req.0,std::ptr::null_mut())==0 { return Err(fail()); }
        let mut status=0u32;let mut len=4;
        if WinHttpQueryHeaders(req.0,WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER,std::ptr::null(),&mut status as *mut _ as _,&mut len,std::ptr::null_mut())==0 { return Err(fail()); }
        if status != 200 { return Ok(Reply{status,body:Value::Null}); }
        // A stateful server cannot be supported by pretending the session is absent.
        let mut session_size=0;
        WinHttpQueryHeaders(req.0,WINHTTP_QUERY_CUSTOM,wide("Mcp-Session-Id").as_ptr(),std::ptr::null_mut(),&mut session_size,std::ptr::null_mut());
        if session_size>0 { return Err("STATEFUL_MCP_UNSUPPORTED".into()); }
        let mut bytes=vec![];let mut chunk=[0u8;8192];let started=std::time::Instant::now();
        loop {
            if started.elapsed().as_secs()>90 { return Err("TIMEOUT".into()); }
            let mut got=0;
            if WinHttpReadData(req.0,chunk.as_mut_ptr() as _,8192,&mut got)==0 { return Err(fail()); }
            if got==0 { break; }
            if bytes.len()+got as usize>1048576 { return Err("RESPONSE_TOO_LARGE".into()); }
            bytes.extend_from_slice(&chunk[..got as usize]);
        }
        let body=serde_json::from_slice(&bytes).map_err(|_|"SSE_OR_INVALID_RESPONSE_UNSUPPORTED")?;
        Ok(Reply{status,body})
    }
}
#[cfg(not(windows))]
pub fn buffer_mcp(_:Value)->Result<Reply,String>{Err("WINDOWS_REQUIRED".into())}
