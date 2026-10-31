//! Outgoing-only contract. Production transmission is deliberately absent.
//! Mock uses an owned short-lived listener on a fixed loopback IP, never user URLs.
use serde_json::{json, Value};
use std::{io::{Read, Write}, net::{TcpListener, TcpStream, SocketAddr}, time::Duration};

pub const MOCK_URL: &str = "http://127.0.0.1:18791/connector-hub/mock";
pub const SCHEMA: &str = "core-handoff-v1";

pub fn validate_url(url: &str, mock: bool) -> Result<(), String> {
    if mock { return if url == MOCK_URL { Ok(()) } else { Err("MOCK_ENDPOINT_MISMATCH".into()) }; }
    if url.len()>2048 || url.contains(['?', '#', '@', '\\', '%']) || url.chars().any(|c|c.is_control() || c.is_whitespace()) {
        return Err("WEBHOOK_URL_REJECTED".into());
    }
    let rest=url.strip_prefix("https://").ok_or("HTTPS_REQUIRED")?;
    let (host,path)=rest.split_once('/').ok_or("WEBHOOK_PATH_REQUIRED")?;
    // Exact reviewed hosts only. No wildcard hosts, IP literals, ports or DNS fallback.
    if !["hook.eu1.make.com","hook.eu2.make.com","hook.us1.make.com","hook.us2.make.com"].contains(&host)
        || path.is_empty() || !path.bytes().all(|b|b.is_ascii_alphanumeric() || b"/-_".contains(&b)) {
        return Err("HOST_REVIEW_REQUIRED".into());
    }
    Ok(())
}

pub fn payload(connector_id: &str, attempt_id: &str) -> Value {
    json!({"schema":SCHEMA,"connectorId":connector_id,"operation":"localMockOnly","attemptId":attempt_id,
        "idempotencyKey":attempt_id,"data":{"text":"ローカル受付試験。外部投稿なし。"}})
}

/// Only synthetic, fixed data crosses the local socket. Caller records intent first.
pub fn send_mock(body: &Value, auth: &str, scenario: &str, timeout_ms: u64) -> Result<Value, String> {
    if !["none","syntheticBearer"].contains(&auth) || !["accepted","httpError","timeout","unknown"].contains(&scenario)
        || body["schema"] != SCHEMA || body["operation"] != "localMockOnly" { return Err("INVALID_MOCK_REQUEST".into()); }
    let listener=TcpListener::bind("127.0.0.1:18791").map_err(|_|"MOCK_PORT_UNAVAILABLE")?;
    listener.set_nonblocking(true).map_err(|_|"MOCK_LISTENER_FAILED")?;
    let bytes=serde_json::to_vec(body).map_err(|_|"INVALID_SCHEMA")?;
    let scenario=scenario.to_string();let expected=bytes.clone();let expected_auth=auth.to_string();
    let server=std::thread::spawn(move || -> Result<(),String> {
        let start=std::time::Instant::now();
        let mut socket=loop {
            match listener.accept() {
                Ok((s,remote)) if remote.ip().is_loopback() => break s,
                Ok(_) => return Err("MOCK_PEER_REJECTED".into()),
                Err(e) if e.kind()==std::io::ErrorKind::WouldBlock && start.elapsed()<Duration::from_secs(3) => std::thread::sleep(Duration::from_millis(5)),
                _ => return Err("MOCK_ACCEPT_FAILED".into()),
            }
        };
        socket.set_nonblocking(false).map_err(|_|"MOCK_SOCKET_MODE")?;
        socket.set_read_timeout(Some(Duration::from_secs(2))).map_err(|_|"MOCK_TIMEOUT_SETTING")?;
        socket.set_write_timeout(Some(Duration::from_secs(2))).map_err(|_|"MOCK_TIMEOUT_SETTING")?;
        let mut request=vec![];let mut part=[0u8;2048];
        loop {
            let n=socket.read(&mut part).map_err(|_|"MOCK_REQUEST_FAILED")?;
            if n==0 || request.len()+n>16384 { return Err("MOCK_REQUEST_INVALID".into()); }
            request.extend_from_slice(&part[..n]);
            if let Some(offset)=request.windows(4).position(|w|w==b"\r\n\r\n") {
                if request.len() >= offset+4+expected.len() {
                    let h=std::str::from_utf8(&request[..offset]).map_err(|_|"INVALID_HEADERS")?;
                    if !h.starts_with("POST /connector-hub/mock HTTP/1.1\r\nHost: 127.0.0.1:18791\r\n") || request[offset+4..]!=expected {
                        return Err("MOCK_PAYLOAD_MISMATCH".into());
                    }
                    if h.contains("Authorization:") != (expected_auth=="syntheticBearer") { return Err("MOCK_AUTH_MISMATCH".into()); }
                    break;
                }
            }
        }
        let response=match scenario.as_str() {
            "timeout" => { std::thread::sleep(Duration::from_millis(timeout_ms+100)); return Ok(()); },
            "httpError" => "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "unknown" => "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
            _ => "HTTP/1.1 202 Accepted\r\nContent-Length: 17\r\nConnection: close\r\n\r\n{\"accepted\":true}",
        };
        socket.write_all(response.as_bytes()).map_err(|_|"MOCK_RESPONSE_FAILED".into())
    });
    let result: Result<Value,&str>=(|| {
        let address:SocketAddr="127.0.0.1:18791".parse().unwrap();
        let mut socket=TcpStream::connect_timeout(&address,Duration::from_millis(timeout_ms)).map_err(|_|"TRANSPORT_UNKNOWN")?;
        if socket.peer_addr().map_err(|_|"MOCK_PEER_REJECTED")? != address { return Err("MOCK_PEER_REJECTED".into()); }
        socket.set_read_timeout(Some(Duration::from_millis(timeout_ms))).map_err(|_|"TIMEOUT_SETTING")?;
        socket.set_write_timeout(Some(Duration::from_millis(timeout_ms))).map_err(|_|"TIMEOUT_SETTING")?;
        let auth_header=if auth=="syntheticBearer" { "Authorization: Bearer local-synthetic-not-a-credential\r\n" } else { "" };
        let headers=format!("POST /connector-hub/mock HTTP/1.1\r\nHost: 127.0.0.1:18791\r\nContent-Type: application/json\r\n{auth_header}Content-Length: {}\r\nConnection: close\r\n\r\n",bytes.len());
        socket.write_all(headers.as_bytes()).and_then(|_|socket.write_all(&bytes)).map_err(|_|"TRANSPORT_UNKNOWN")?;
        let mut response=vec![];let mut part=[0u8;1024];
        loop {
            let n=socket.read(&mut part).map_err(|e|match e.kind(){std::io::ErrorKind::TimedOut|std::io::ErrorKind::WouldBlock=>"TIMEOUT",_=>"TRANSPORT_UNKNOWN"})?;
            if n==0 {break;}
            response.extend_from_slice(&part[..n]);
            if response.len()>4096 {return Err("MOCK_RESPONSE_TOO_LARGE");}
            if let Some(offset)=response.windows(4).position(|w|w==b"\r\n\r\n") {
                let headers=std::str::from_utf8(&response[..offset]).map_err(|_|"TRANSPORT_UNKNOWN")?;
                let size=headers.lines().find_map(|l|l.strip_prefix("Content-Length: ")).and_then(|s|s.parse::<usize>().ok()).ok_or("TRANSPORT_UNKNOWN")?;
                if size>1024 {return Err("MOCK_RESPONSE_TOO_LARGE");}
                if response.len()==offset+4+size {break;}
            }
        }
        let response=String::from_utf8(response).map_err(|_|"TRANSPORT_UNKNOWN")?;
        if response.starts_with("HTTP/1.1 202 ") && response.ends_with("{\"accepted\":true}") {
            Ok(json!({"httpStatus":202,"receipt":"ACCEPTED","publication":"NOT_VERIFIED"}))
        } else if response.starts_with("HTTP/1.1 500 ") {
            Ok(json!({"httpStatus":500,"receipt":"FAILED","publication":"NOT_VERIFIED"}))
        } else { Ok(json!({"httpStatus":200,"receipt":"UNKNOWN","publication":"NOT_VERIFIED"})) }
    })();
    let server_result=server.join().map_err(|_|"MOCK_SERVER_FAILED")?;
    // Transport result unknown must stay unknown even if server saw the payload.
    if result.is_ok() { server_result?; }
    result.map_err(String::from)
}
