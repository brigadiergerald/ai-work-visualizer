//! Per-user product configuration. Credentials remain owned by Codex.
use crate::{engine::{Engine,safe_project},model::*,rpc};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{path::Path,sync::atomic::Ordering};

#[derive(Clone,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase",default)]
pub struct Settings {
 pub setup_complete:bool,pub codex_path:String,pub work_root:String,pub output_root:String,
 pub model:String,pub approval_policy:String,
 pub input_paths:Vec<String>,
}
impl Settings {
 pub fn validate(&self)->Result<(),String>{
  safe_project(self.work_root.clone())?;safe_project(self.output_root.clone())?;
  if !self.codex_path.is_empty(){validate_exe(&self.codex_path)?;}
  if self.approval_policy!="on-request"{return Err("この版はon-request承認・読み取り専用実行です".into())}
  if self.model.len()>200||self.model.chars().any(char::is_control){return Err("モデル指定が不正です".into())}Ok(())
 }
}
pub fn validate_exe(path:&str)->Result<(),String>{let p=Path::new(path);if !p.is_absolute()||!p.is_file()||p.file_name().and_then(|s|s.to_str()).is_none_or(|s|!s.eq_ignore_ascii_case("codex.exe")){return Err("Codex本体のcodex.exeを既存の絶対パスで指定してください".into())}Ok(())}
pub fn check_version(v:&str)->Result<(),String>{
 let re=regex::Regex::new(r"(?:codex(?:-cli)?\s+)?(\d+)\.(\d+)\.(\d+)").unwrap();
 let c=re.captures(v).ok_or("Codexバージョンを判別できません。対応版を確認してください")?;
 let major=c[1].parse::<u64>().map_err(|e|e.to_string())?;let minor=c[2].parse::<u64>().map_err(|e|e.to_string())?;
 if major==0&&minor<114{return Err("Codex 0.114.0以降のapp-serverが必要です。更新後に接続を再確認してください".into())}Ok(())
}
// Exact installed protocols verified for OPEN-CORE-07 and CODEX-0166-COMPAT.
// Keep the effective config/thread guards in rpc.rs; unknown builds stop before spawn.
pub fn check_readonly_version(v:&str)->Result<(),String>{
 if !matches!(v.trim(),"codex-cli 0.164.0-alpha.11.1"|"codex-cli 0.166.0-alpha.5.1"){return Err("このCodex版の標準read-only契約は未検証です。権限を変更せず接続を停止します。".into())}Ok(())
}
pub fn inspect(path:&str)->Value{
 let result=(||{let exe=if path.trim().is_empty(){rpc::codex_path()?}else{validate_exe(path)?;path.into()};
  let output=rpc::command(&exe).arg("--version").output().map_err(|e|format!("Codexを起動できません: {e}"))?;
  if !output.status.success(){return Err("codex --versionに失敗しました".into())}
  let version=String::from_utf8_lossy(&output.stdout).trim().to_string();check_version(&version)?;
  Ok::<_,String>(json!({"available":true,"path":exe,"version":version,"protocol":"接続テストで確認","auth":"未確認","entitlement":"実推論による利用権限は未確認","inferencePerformed":false}))})();
 result.unwrap_or_else(|e|json!({"available":false,"error":redact(&e),"inferencePerformed":false}))
}
impl Engine {
 pub fn save_setup(&self,mut settings:Settings)->Result<(),String>{
  if self.worker.load(Ordering::SeqCst)||self.connecting.load(Ordering::SeqCst){return Err("実行・接続確認の終了後に設定してください".into())}
  let s=self.snapshot();if s.job.as_ref().is_some_and(|j|["paused","disconnected","awaiting_approval","running","handing_off"].contains(&j.status.as_str())){return Err("元Jobの停止または復旧後に設定を変更してください".into())}
  settings.validate()?;settings.setup_complete=true;
  settings.work_root=safe_project(settings.work_root)?.to_string_lossy().trim_start_matches("\\\\?\\").into();
  settings.output_root=safe_project(settings.output_root)?.to_string_lossy().trim_start_matches("\\\\?\\").into();
  if let Some(r)=self.rpc.lock().unwrap().take(){r.shutdown();}
  self.update(|s|{s.project=Project{name:"マイワークスペース".into(),path:settings.work_root.clone()};if !s.projects.iter().any(|p|p.path==s.project.path){s.projects.push(s.project.clone())}s.settings=settings;s.connection.status="disconnected".into();s.connection.error=None;s.event(None,"setup_saved","queued","作業範囲・納品先・承認方針を保存。認証情報は保存していません。接続テストは未実行です。",None,vec![]);Ok(())})
 }
 pub fn connection_details(&self)->Result<Value,String>{
  let rpc=self.get_rpc()?;let account=rpc.call("account/read",json!({"refreshToken":false}))?;
  let mut models=vec![];let mut cursor=Value::Null;
  for _ in 0..20 {let mut p=json!({"limit":100});if !cursor.is_null(){p["cursor"]=cursor.clone();}let r=rpc.call("model/list",p)?;
   for model in r["data"].as_array().ok_or("モデル一覧が不正です")?{if let Some(id)=model["model"].as_str().or(model["id"].as_str()){models.push(json!({"id":id,"name":model["displayName"],"isDefault":model["isDefault"]}));}}
   cursor=r["nextCursor"].clone();if cursor.is_null(){break}
  }
  let limits=rpc.call("account/rateLimits/read",json!({})).ok().map(|v|json!({"rateLimits":v["rateLimits"],"rateLimitsByLimitId":v["rateLimitsByLimitId"]}));
  Ok(json!({"authType":account["account"]["type"],"planType":account["account"]["planType"],"models":models,"limits":limits,"modelEntitlement":"NOT_VERIFIED_WITHOUT_INFERENCE","commercialAuthorization":"NOT_APPROVED","inferencePerformed":false}))
 }
}
