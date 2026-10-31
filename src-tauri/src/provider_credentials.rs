//! Trusted host only. No API exposes secret bytes, headers, or source contents.
use serde::{Deserialize,Serialize};
use std::{fs,path::Path};

#[derive(Clone,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="camelCase",deny_unknown_fields)]
pub enum Source {
 ExistingEnv {path:String,field:String},
 NativeVault,
 Mock,
}
// Deliberately no Debug, Serialize or Clone on plaintext.
pub struct Secret(Vec<u8>);
impl Secret {
 pub fn bytes(&self)->&[u8]{&self.0}
 pub fn from_bytes(bytes:Vec<u8>)->Result<Self,String>{
  let s=Self(bytes);
  if s.0.is_empty()||s.0.len()>16384||!s.0.iter().all(|b|b.is_ascii_graphic()){return Err("資格情報の形式が不正です。元の設定を確認してください".into())}Ok(s)
 }
}
impl Drop for Secret {fn drop(&mut self){for b in &mut self.0 {unsafe{std::ptr::write_volatile(b,0)}}}}

// Webhook paths can themselves contain secrets. Keep the full validated URL in
// the existing OS vault; SQLite/dashboard receive only a host and endpoint ref.
#[cfg(windows)]
pub(crate) fn protect_connector_endpoint(reference:&str,endpoint:&str)->Result<(),String>{
 if !reference.starts_with("ai-guild/connector/webhook/") {return Err("Endpoint資格情報領域が不正です".into())}
 crate::connector_webhook::validate_url(endpoint,false)?;
 write_vault(reference,Secret::from_bytes(endpoint.as_bytes().to_vec())?)
}
#[cfg(not(windows))]
pub(crate) fn protect_connector_endpoint(_:&str,_:&str)->Result<(),String>{Err("Windows資格情報保護が必要です".into())}

pub fn validate_source(provider:&str,source:&Source)->Result<(),String>{
 match (provider,source) {
  ("buffer",Source::ExistingEnv{path,field})=>{
   let p=Path::new(path);
   if !p.is_absolute()||p.file_name().is_none_or(|n|n!=".env")||field!="BUFFER_API_KEY" {return Err("明示選択した.envのBUFFER_API_KEY参照だけを使用できます".into())}
   if fs::metadata(p).map_err(|_|"資格情報の参照先を利用できません")?.len()>1048576{return Err("資格情報ファイルが大きすぎます".into())}Ok(())
  },
  ("buffer",Source::NativeVault)|("mock",Source::Mock)=>Ok(()),
  _=>Err("Providerと資格情報の方式が一致しません".into())
 }
}
pub fn resolve(provider:&str,reference:&str,source:&Source)->Result<Secret,String>{
 validate_source(provider,source)?;
 match source {
  Source::ExistingEnv{path,field}=>{
   // Read-only. Never imports/copies this existing credential to another store.
   let mut text=fs::read(path).map_err(|_|"資格情報を参照できません。元のファイルの権限を確認してください")?;
   let parsed=(||{
    let t=std::str::from_utf8(&text).map_err(|_|"資格情報ファイルの文字コードが未対応です")?;
    let mut matches=t.trim_start_matches('\u{feff}').lines().filter_map(|l|l.trim().strip_prefix("export ").unwrap_or(l.trim()).split_once('=')).filter(|(k,_)|k.trim()==field);
    let (_,v)=matches.next().ok_or("指定した認証項目がありません")?;
    if matches.next().is_some(){return Err("認証項目が重複しています".into())}
    let v=v.trim();let v=if v.len()>1&&((v.starts_with('"')&&v.ends_with('"'))||(v.starts_with('\'')&&v.ends_with('\''))){&v[1..v.len()-1]}else{v};
    Secret::from_bytes(v.as_bytes().to_vec())
   })();
   for b in &mut text{unsafe{std::ptr::write_volatile(b,0)}}parsed
  },
  Source::NativeVault=>read_vault(reference),
  Source::Mock=>Err("模擬Adapterには実資格情報を渡しません".into())
 }
}
#[cfg(windows)]
pub fn enroll_vault(reference:&str)->Result<(),String>{
 use windows_sys::Win32::Security::Credentials::*;
 let target=wide(reference);let caption=wide("AI GUILD · API資格情報の登録");
 let message=wide("APIキーはWindows資格情報に保存します。ユーザー名は識別用です。パスワード欄へキーを入力してください。");
 let info=CREDUI_INFOW{cbSize:std::mem::size_of::<CREDUI_INFOW>() as u32,hwndParent:std::ptr::null_mut(),pszMessageText:message.as_ptr(),pszCaptionText:caption.as_ptr(),hbmBanner:std::ptr::null_mut()};
 let mut user=vec![0u16;514];let name=wide("buffer");user[..name.len()].copy_from_slice(&name);let mut pass=vec![0u16;16385];let mut save=0;
 let result=unsafe{CredUIPromptForCredentialsW(&info,target.as_ptr(),std::ptr::null(),0,user.as_mut_ptr(),user.len() as u32,pass.as_mut_ptr(),pass.len() as u32,&mut save,CREDUI_FLAGS_GENERIC_CREDENTIALS|CREDUI_FLAGS_DO_NOT_PERSIST|CREDUI_FLAGS_ALWAYS_SHOW_UI|CREDUI_FLAGS_EXCLUDE_CERTIFICATES)};
 let stored=(||{
  if result!=0{return Err("資格情報の登録を完了できませんでした。入力の取り消しや権限を確認してください".into())}
  let len=pass.iter().position(|b|*b==0).unwrap_or(pass.len());
  // Buffer bearer keys are ASCII. Validate without exposing through the webview.
  if len==0||len>16384||!pass[..len].iter().all(|c|(33..=126).contains(c)){return Err("APIキーの形式が不正です".into())}
  write_vault(reference,Secret::from_bytes(pass[..len].iter().map(|b|*b as u8).collect())?)
 })();
 for c in &mut pass{unsafe{std::ptr::write_volatile(c,0)}}stored
}
#[cfg(windows)]
fn write_vault(reference:&str,mut secret:Secret)->Result<(),String>{
 use windows_sys::Win32::Security::Credentials::*;
 let target=wide(reference);let mut user=wide("api-key");
 let credential=CREDENTIALW{Flags:0,Type:CRED_TYPE_GENERIC,TargetName:target.as_ptr() as *mut _,Comment:std::ptr::null_mut(),LastWritten:unsafe{std::mem::zeroed()},CredentialBlobSize:secret.0.len() as u32,CredentialBlob:secret.0.as_mut_ptr(),Persist:CRED_PERSIST_LOCAL_MACHINE,AttributeCount:0,Attributes:std::ptr::null_mut(),TargetAlias:std::ptr::null_mut(),UserName:user.as_mut_ptr()};
 if unsafe{CredWriteW(&credential,0)}==0{return Err("Windows資格情報への保存に失敗しました".into())}Ok(())
}
#[cfg(windows)]
fn read_vault(reference:&str)->Result<Secret,String>{
 use windows_sys::Win32::Security::Credentials::*;
 let mut c=std::ptr::null_mut();let target=wide(reference);
 unsafe{
  if CredReadW(target.as_ptr(),CRED_TYPE_GENERIC,0,&mut c)==0{return Err("Windows資格情報が利用できません。再登録してください".into())}
  let result=Secret::from_bytes(std::slice::from_raw_parts((*c).CredentialBlob,(*c).CredentialBlobSize as usize).to_vec());
  for i in 0..(*c).CredentialBlobSize as usize{std::ptr::write_volatile((*c).CredentialBlob.add(i),0)}CredFree(c as _);result
 }
}
#[cfg(not(windows))]pub fn enroll_vault(_: &str)->Result<(),String>{Err("Windows資格情報保護が必要です".into())}
#[cfg(not(windows))]fn read_vault(_: &str)->Result<Secret,String>{Err("Windows資格情報保護が必要です".into())}
#[cfg(windows)]fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}

#[cfg(all(test,windows))]mod tests{
 use super::*;
 #[test]fn provider_windows_vault_synthetic_roundtrip_only(){
  use windows_sys::Win32::Security::Credentials::*;
  let reference=format!("ai-guild/test-only/{}",crate::model::id());
  write_vault(&reference,Secret::from_bytes(b"synthetic-offline-key-not-an-account".to_vec()).unwrap()).unwrap();
  let result=read_vault(&reference);let target=wide(&reference);
  let deleted=unsafe{CredDeleteW(target.as_ptr(),CRED_TYPE_GENERIC,0)};assert_ne!(deleted,0);
  assert_eq!(result.unwrap().bytes(),b"synthetic-offline-key-not-an-account");assert!(read_vault(&reference).is_err());
 }
}
