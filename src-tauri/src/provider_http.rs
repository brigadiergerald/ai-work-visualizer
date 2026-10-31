//! Narrow native transport: fixed HTTPS host, no redirect, no automatic retry.
use crate::provider_credentials::Secret;
pub struct Reply {pub status:u32,pub bytes:Vec<u8>}
#[derive(Clone,Copy,Debug)]pub enum Error {Timeout,Transport,TooLarge}
pub trait ReadTransport {fn query(&mut self,body:&[u8])->Result<Reply,Error>;}
pub struct BufferTransport {secret:Secret}
impl BufferTransport {pub fn new(secret:Secret)->Self{Self{secret}}}
impl ReadTransport for BufferTransport {
 fn query(&mut self,body:&[u8])->Result<Reply,Error>{
  if !crate::provider_buffer::approved_body(body){return Err(Error::Transport)}
  request(&self.secret,body)
 }
}
#[cfg(windows)]
pub(crate) fn request(secret:&Secret,body:&[u8])->Result<Reply,Error>{
  use windows_sys::Win32::{Networking::WinHttp::*,Foundation::GetLastError};
  struct Handle(*mut std::ffi::c_void);
  impl Drop for Handle {fn drop(&mut self){unsafe{WinHttpCloseHandle(self.0);}}}
  fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}
  fn failure()->Error{if unsafe{GetLastError()}==12002{Error::Timeout}else{Error::Transport}}
  unsafe{
   let agent=wide("AI-Work-Visualizer-Core/0.1");
   let session=Handle(WinHttpOpen(agent.as_ptr(),WINHTTP_ACCESS_TYPE_NO_PROXY,std::ptr::null(),std::ptr::null(),0));if session.0.is_null(){return Err(failure())}
   if WinHttpSetTimeouts(session.0,45000,45000,45000,45000)==0{return Err(failure())}
   let attempts=1u32;
   if WinHttpSetOption(session.0,WINHTTP_OPTION_CONNECT_RETRIES,&attempts as *const _ as _,4)==0{return Err(failure())}
   let retries=WINHTTP_FAILED_CONNECTION_RETRIES{dwMaxRetries:0,dwAllowedRetryConditions:0};
   if WinHttpSetOption(session.0,WINHTTP_OPTION_FAILED_CONNECTION_RETRIES,&retries as *const _ as _,std::mem::size_of_val(&retries) as u32)==0{return Err(failure())}
   let host=wide("api.buffer.com");let conn=Handle(WinHttpConnect(session.0,host.as_ptr(),443,0));if conn.0.is_null(){return Err(failure())}
   let verb=wide("POST");let path=wide("/");let req=Handle(WinHttpOpenRequest(conn.0,verb.as_ptr(),path.as_ptr(),std::ptr::null(),std::ptr::null(),std::ptr::null(),WINHTTP_FLAG_SECURE));if req.0.is_null(){return Err(failure())}
   let policy=WINHTTP_OPTION_REDIRECT_POLICY_NEVER;
   if WinHttpSetOption(req.0,WINHTTP_OPTION_REDIRECT_POLICY,&policy as *const _ as _,4)==0{return Err(failure())}
   let disabled=WINHTTP_DISABLE_COOKIES|WINHTTP_DISABLE_AUTHENTICATION|WINHTTP_DISABLE_KEEP_ALIVE;
   if WinHttpSetOption(req.0,WINHTTP_OPTION_DISABLE_FEATURE,&disabled as *const _ as _,4)==0{return Err(failure())}
   let mut header:Vec<u16>="Content-Type: application/json\r\nAuthorization: Bearer ".encode_utf16().chain(secret.bytes().iter().map(|b|*b as u16)).chain("\r\n".encode_utf16()).chain(Some(0)).collect();
   let sent=WinHttpSendRequest(req.0,header.as_ptr(),(header.len()-1) as u32,body.as_ptr() as _,body.len() as u32,body.len() as u32,0);
   for c in &mut header{std::ptr::write_volatile(c,0)}
   if sent==0{return Err(failure())}if WinHttpReceiveResponse(req.0,std::ptr::null_mut())==0{return Err(failure())}
   let mut status=0u32;let mut len=4;
   if WinHttpQueryHeaders(req.0,WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER,std::ptr::null(),&mut status as *mut _ as _,&mut len,std::ptr::null_mut())==0{return Err(failure())}
   // Non-success bodies may echo secrets. Never read them into evidence.
   if status!=200{return Ok(Reply{status,bytes:vec![]})}
   let mut bytes=Vec::new();let mut chunk=[0u8;8192];let reading=std::time::Instant::now();
   loop{if reading.elapsed()>std::time::Duration::from_secs(90){return Err(Error::Timeout)}let mut got=0;if WinHttpReadData(req.0,chunk.as_mut_ptr() as _,chunk.len() as u32,&mut got)==0{return Err(failure())}if got==0{break}if bytes.len()+got as usize>1048576{return Err(Error::TooLarge)}bytes.extend_from_slice(&chunk[..got as usize]);}
   if bytes.windows(secret.bytes().len()).any(|b|b==secret.bytes()){for b in &mut bytes{std::ptr::write_volatile(b,0)}return Err(Error::Transport)}
   Ok(Reply{status,bytes})
  }
}
#[cfg(not(windows))]pub(crate) fn request(_:&Secret,_:&[u8])->Result<Reply,Error>{Err(Error::Transport)}
