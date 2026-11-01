//! Optional handoff domain v1. No network, credentials, browser, or process executor.
use crate::{engine::{Engine,verify_artifact},model::{id,now,redact,Job}};
use rusqlite::{params,Connection,OptionalExtension};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,io::Write,path::{Path,PathBuf},collections::HashSet};

#[derive(Clone,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub enum Goal {#[default] LocalFiles,ExportPackage,ExternalDraft,Scheduled,PublishedVerified}
impl Goal {fn external(&self)->bool{!matches!(self,Self::LocalFiles|Self::ExportPackage)}}
#[derive(Clone,Default,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PlanInput {pub goal:Goal,pub profile_id:Option<String>,pub profile_revision:Option<u64>}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Profile {
 pub id:String,pub revision:u64,pub label:String,pub provider:String,pub transport:String,
 pub endpoint:String,pub account_label:String,pub credential_ref:Option<String>,pub capabilities:Vec<Goal>,
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct BoundPlan {pub goal:Goal,pub profile:Option<Profile>,pub selected_at:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub enum Scenario {Success,AuthenticationFailure,Unsupported,Timeout,Processing,WrongAccount}
#[derive(Deserialize)]
#[serde(tag="operation",rename_all="camelCase",rename_all_fields="camelCase",deny_unknown_fields)]
pub enum Action {
 Get{job_id:Option<String>},SaveProfile{profile:Profile},
 MockCheck{profile_id:String,profile_revision:u64,goal:Goal,scenario:Scenario},
 Export{job_id:String,expected_digest:String},VerifyExport{delivery_id:String},
 PrepareExternal{job_id:String,expected_digest:String,plan:PlanInput},
}
#[derive(Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct PackageFile {pub name:String,pub sha256:String,pub bytes:u64}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
struct Delivery {
 id:String,job_id:String,source_digest:String,plan:BoundPlan,status:String,created_at:String,
 folder:Option<String>,files:Vec<PackageFile>,manifest:Option<Value>,manifest_sha256:Option<String>,history:Vec<Value>,
}
fn hash(b:&[u8])->String{crate::hashing::hex(&Sha256::digest(b))}
fn init(db:&Connection)->Result<(),String>{db.execute_batch(
 "CREATE TABLE IF NOT EXISTS integration_profiles(id TEXT PRIMARY KEY,revision INTEGER NOT NULL,json TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS integration_checks(id TEXT PRIMARY KEY,profile_id TEXT NOT NULL,json TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS delivery_runs(id TEXT PRIMARY KEY,dedupe TEXT UNIQUE NOT NULL,json TEXT NOT NULL);").map_err(|e|e.to_string())}
fn text_ok(s:&str,max:usize)->bool{!s.trim().is_empty()&&s.chars().count()<=max&&!s.chars().any(char::is_control)&&redact(s)==s}
fn valid_profile(p:&Profile)->Result<(),String>{
 uuid::Uuid::parse_str(&p.id).map_err(|_|"接続設定のIDが不正です")?;
 if !text_ok(&p.label,80)||!text_ok(&p.account_label,100){return Err("接続名とアカウント識別ラベルを入力してください。秘密値は保存できません".into())}
 if !["buffer","youtube","postmesh","custom"].contains(&p.provider.as_str())||!["api","mcp","webhook"].contains(&p.transport.as_str()){return Err("接続先または方式が未対応です".into())}
 let url=tauri::Url::parse(&p.endpoint).map_err(|_|"接続先はhttpsのURLで入力してください")?;
 if url.scheme()!="https"||url.host_str().is_none()||!url.username().is_empty()||url.password().is_some()||url.query().is_some()||url.fragment().is_some()||p.endpoint.len()>2048||redact(&p.endpoint)!=p.endpoint{return Err("接続先は認証値・クエリ・fragmentを含まないhttps URLで指定してください".into())}
 if p.credential_ref.as_ref().is_some_and(|s|s.is_empty()||s.len()>80||!s.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_.".contains(&b))||s.starts_with("sk-")||s.starts_with("pm_live_")){return Err("認証参照名は80文字以内の英数字・ハイフン・ピリオド・下線です。キー本体は入力しないでください".into())}
 let mut seen=HashSet::new();
 if p.capabilities.is_empty()||p.capabilities.iter().any(|g|!g.external()||!seen.insert(serde_json::to_string(g).unwrap())){return Err("予定する外部操作を重複なく選択してください".into())}Ok(())
}
fn profile(db:&Connection,id:&str)->Result<Profile,String>{
 let raw:String=db.query_row("SELECT json FROM integration_profiles WHERE id=?1",[id],|r|r.get(0)).map_err(|_|"接続設定がありません")?;
 let p:Profile=serde_json::from_str(&raw).map_err(|e|e.to_string())?;valid_profile(&p)?;if p.id!=id{return Err("接続設定の所属が不正です".into())}Ok(p)
}
fn bind(db:&Connection,p:PlanInput)->Result<BoundPlan,String>{
 let target=if p.goal.external(){
  let target=profile(db,p.profile_id.as_deref().ok_or("接続先を選択してください")?)?;
  if Some(target.revision)!=p.profile_revision{return Err("接続設定が更新されています。再選択してください".into())}
  if !target.capabilities.contains(&p.goal){return Err("この接続設定では選択した操作を予定していません".into())}Some(target)
 }else{if p.profile_id.is_some()||p.profile_revision.is_some(){return Err("ローカル出力に接続先は不要です".into())}None};
 Ok(BoundPlan{goal:p.goal,profile:target,selected_at:now()})
}
pub fn bind_plan(e:&Engine,p:PlanInput)->Result<BoundPlan,String>{let d=e.data.lock().map_err(|_|"状態ロックエラー")?;init(&d.store.0)?;bind(&d.store.0,p)}
fn source(e:&Engine,job_id:&str)->Result<(Job,Vec<PackageFile>,String),String>{
 let s=e.snapshot();let j=s.job.iter().chain(s.history.iter()).find(|j|j.job_id==job_id).ok_or("元の制作Jobがありません")?;
 if j.status!="completed"||j.mode=="demo"{return Err("内容確認とローカル納品が完了したJobだけを出力できます".into())}
 crate::practical::verify_product(j)?;crate::sns::approved(j)?;
 let mut names=HashSet::new();let mut files=vec![];
 for a in j.artifacts.iter().filter(|a|a.kind=="product"){
  if a.name.is_empty()||a.name.contains(['/', '\\', ':'])||a.name.contains("..")||a.name.eq_ignore_ascii_case("delivery-package.json")||!names.insert(a.name.to_lowercase()){return Err("出力ファイル名が重複、予約名、または不正です".into())}
  verify_artifact(a,Path::new(&j.workspace))?;files.push(PackageFile{name:a.name.clone(),sha256:a.sha256.clone(),bytes:a.bytes});
 }
 if files.is_empty(){return Err("完成品がありません".into())}
 files.sort_by(|a,b|a.name.cmp(&b.name));let digest=hash(&serde_json::to_vec(&files).map_err(|e|e.to_string())?);Ok((j.clone(),files,digest))
}
fn records(db:&Connection,sql:&str)->Result<Vec<Value>,String>{let mut q=db.prepare(sql).map_err(|e|e.to_string())?;let rows=q.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;rows.map(|r|serde_json::from_str(&r.map_err(|e|e.to_string())?).map_err(|e|e.to_string())).collect()}
fn load_delivery(db:&Connection,id:&str)->Result<Delivery,String>{let raw:String=db.query_row("SELECT json FROM delivery_runs WHERE id=?1",[id],|r|r.get(0)).map_err(|_|"出力記録がありません")?;let r:Delivery=serde_json::from_str(&raw).map_err(|e|e.to_string())?;if r.id!=id{return Err("出力記録の所属が不正です".into())}Ok(r)}
fn save_delivery(db:&Connection,r:&Delivery)->Result<(),String>{db.execute("UPDATE delivery_runs SET json=?1 WHERE id=?2",params![serde_json::to_string(r).map_err(|e|e.to_string())?,r.id]).map_err(|e|e.to_string())?;Ok(())}
fn export_folder(e:&Engine,id:&str)->Result<PathBuf,String>{uuid::Uuid::parse_str(id).map_err(|_|"出力IDが不正です")?;Ok(e.profile_root.join("exports").join(id))}
fn no_link(p:&Path)->Result<(),String>{let m=fs::symlink_metadata(p).map_err(|e|e.to_string())?;
 #[cfg(windows)] {use std::os::windows::fs::MetadataExt;if m.file_attributes()&0x400!=0{return Err("リンク先への出力・照合は行いません".into())}}
 if m.file_type().is_symlink(){return Err("リンク先への出力・照合は行いません".into())}Ok(())}
fn verify_package(e:&Engine,r:&Delivery)->Result<(),String>{
 if r.plan.goal!=Goal::ExportPackage{return Err("ファイル出力の記録ではありません".into())}
 let folder=export_folder(e,&r.id)?;if r.folder.as_deref()!=Some(folder.to_string_lossy().as_ref()){return Err("出力先が記録と一致しません".into())}
 no_link(folder.parent().unwrap())?;no_link(&folder)?;
 if fs::read_dir(&folder).map_err(|e|e.to_string())?.count()!=r.files.len()+1{return Err("出力フォルダの件数がmanifestと一致しません".into())}
 let mut names=HashSet::new();for f in &r.files{
  if f.name.is_empty()||f.name.contains(['/', '\\', ':'])||f.name.contains("..")||!names.insert(&f.name){return Err("出力記録のファイル名が不正です".into())}
  let p=folder.join(&f.name);no_link(&p)?;let b=fs::read(&p).map_err(|e|e.to_string())?;
  if b.len() as u64!=f.bytes||hash(&b)!=f.sha256{return Err(format!("出力ファイルの内容・hashが異なります: {}",f.name))}
 }
 let p=folder.join("delivery-package.json");no_link(&p)?;let b=fs::read(&p).map_err(|e|e.to_string())?;
 if Some(hash(&b))!=r.manifest_sha256{return Err("引き渡しmanifestのhashが一致しません".into())}
 let v:Value=serde_json::from_slice(&b).map_err(|e|e.to_string())?;
 if Some(&v)!=r.manifest.as_ref()||v["jobId"]!=r.job_id||v["sourceDigest"]!=r.source_digest||v["deliveryId"]!=r.id||v["files"]!=json!(r.files){return Err("manifestの所属・本文対応が一致しません".into())}Ok(())
}
fn exact_new(p:&Path,b:&[u8])->Result<(),String>{let mut f=fs::OpenOptions::new().write(true).create_new(true).open(p).map_err(|e|e.to_string())?;f.write_all(b).and_then(|_|f.sync_all()).map_err(|e|e.to_string())?;if fs::read(p).map_err(|e|e.to_string())?!=b{return Err("出力後のbytes照合に失敗しました".into())}Ok(())}
fn export(e:&Engine,db:&Connection,j:&Job,files:Vec<PackageFile>,digest:&str)->Result<Delivery,String>{
 let dedupe=format!("export:{}:{digest}",j.job_id);
 let old:Option<String>=db.query_row("SELECT id FROM delivery_runs WHERE dedupe=?1",[&dedupe],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
 if let Some(id)=old{let r=load_delivery(db,&id)?;if r.status!="EXPORTED"{return Err("元の出力が未完了または結果不明です。出力結果を照合し、重複出力は行いません".into())}verify_package(e,&r)?;return Ok(r)}
 let id=id();let folder=export_folder(e,&id)?;
 let manifest=json!({"version":1,"deliveryId":id,"jobId":j.job_id,"sourceDigest":digest,"files":files,"publication":"NOT_PERFORMED","format":"local-files","createdAt":now()});
 let bytes=serde_json::to_vec_pretty(&manifest).map_err(|e|e.to_string())?;
 let mut r=Delivery{id:id.clone(),job_id:j.job_id.clone(),source_digest:digest.into(),plan:BoundPlan{goal:Goal::ExportPackage,profile:None,selected_at:now()},status:"WRITING_UNKNOWN".into(),created_at:now(),folder:Some(folder.to_string_lossy().into()),files,manifest:Some(manifest),manifest_sha256:Some(hash(&bytes)),history:vec![json!({"at":now(),"state":"WRITING_UNKNOWN","operation":"LOCAL_EXPORT_INTENT"})]};
 db.execute("INSERT INTO delivery_runs VALUES(?1,?2,?3)",params![id,dedupe,serde_json::to_string(&r).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
 let result=(||{
  let parent=folder.parent().unwrap();fs::create_dir_all(parent).map_err(|e|e.to_string())?;no_link(parent)?;fs::create_dir(&folder).map_err(|e|e.to_string())?;
  for f in &r.files{let a=j.artifacts.iter().find(|a|a.kind=="product"&&a.name==f.name).ok_or("元の成果物なし")?;verify_artifact(a,Path::new(&j.workspace))?;let b=fs::read(&a.path).map_err(|e|e.to_string())?;if hash(&b)!=f.sha256||b.len() as u64!=f.bytes{return Err("出力中に元の完成品が変化しました".into())}exact_new(&folder.join(&f.name),&b)?;}
  exact_new(&folder.join("delivery-package.json"),&bytes)?;verify_package(e,&r)
 })();
 r.status=if result.is_ok(){"EXPORTED"}else{"FAILED"}.into();
 r.history.push(json!({"at":now(),"state":r.status,"operation":"LOCAL_EXPORT_CHECK","error":result.as_ref().err().map(|s|redact(s))}));save_delivery(db,&r)?;result?;Ok(r)
}
fn external(db:&Connection,j:&Job,digest:&str,plan:BoundPlan)->Result<Delivery,String>{
 if !plan.goal.external(){return Err("外部連携の到達目標を選択してください".into())}
 let p=plan.profile.as_ref().ok_or("接続先なし")?;let dedupe=format!("external:{}:{digest}:{}:{}:{:?}",j.job_id,p.id,p.revision,plan.goal);
 let old:Option<String>=db.query_row("SELECT id FROM delivery_runs WHERE dedupe=?1",[&dedupe],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
 if let Some(id)=old{return load_delivery(db,&id)}
 let r=Delivery{id:id(),job_id:j.job_id.clone(),source_digest:digest.into(),plan,status:"BLOCKED_UNIMPLEMENTED".into(),created_at:now(),folder:None,files:vec![],manifest:None,manifest_sha256:None,history:vec![json!({"at":now(),"state":"BLOCKED_UNIMPLEMENTED","operation":"PREPARE_ONLY","message":"接続設定を固定しました。実接続・認証・送信は次工程です。"})]};
 db.execute("INSERT INTO delivery_runs VALUES(?1,?2,?3)",params![r.id,dedupe,serde_json::to_string(&r).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;Ok(r)
}
pub fn perform(e:&Engine,a:Action)->Result<Value,String>{
 // Snapshot/verify before the profile mutex. Completed source Jobs are never edited.
 let job_id=match &a{Action::Get{job_id}=>job_id.clone(),Action::Export{job_id,..}|Action::PrepareExternal{job_id,..}=>Some(job_id.clone()),_=>None};
 let checked=job_id.as_ref().map(|id|source(e,id));
 let src=checked.as_ref().and_then(|v|v.as_ref().ok());let source_error=checked.as_ref().and_then(|v|v.as_ref().err()).cloned();
 // view needs its own immutable snapshot; do not reacquire Engine::data under this lock.
 let snapshot=e.snapshot();
 let d=e.data.lock().map_err(|_|"状態ロックエラー")?;let db=&d.store.0;init(db)?;
 match a {
  Action::Get{..}=>view_with_snapshot(e,db,job_id.as_deref(),src,source_error,&snapshot),
  Action::SaveProfile{profile:mut p}=>{
   valid_profile(&p)?;
   let old:Option<u64>=db.query_row("SELECT revision FROM integration_profiles WHERE id=?1",[&p.id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
   if old.unwrap_or(0)!=p.revision{return Err("設定の版が変わっています。読み直してください".into())}
   p.revision=p.revision.checked_add(1).ok_or("設定revision上限")?;
   db.execute("INSERT INTO integration_profiles VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,json=excluded.json",params![p.id,p.revision,serde_json::to_string(&p).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
   Ok(json!({"profile":p,"connectionStatus":"NOT_TESTED","message":"設定を保存しました。実接続・認証・投稿は行っていません。"}))
  },
  Action::MockCheck{profile_id,profile_revision,goal,scenario}=>{
   let p=profile(db,&profile_id)?;if p.revision!=profile_revision{return Err("設定の版が変わっています。読み直してください".into())}
   let supported=p.capabilities.contains(&goal);let status=if !supported{"FAILED"}else{match scenario{Scenario::Success=>"PASS",Scenario::Timeout=>"UNKNOWN",Scenario::Processing=>"IN_PROGRESS",_=>"FAILED"}};
   let result=json!({"id":id(),"profileId":profile_id,"profileRevision":profile_revision,"goal":goal,"scenario":scenario,"at":now(),"scope":"OFFLINE_SIMULATION","status":status,"supportedByConfiguration":supported,"connectionStatus":"NOT_TESTED","accountCheck":"NOT_VERIFIED","networkRequests":0,"externalWrites":0,"message":"模擬応答の検査結果です。実サービスの接続・認証・操作成功は未確認です。"});
   db.execute("INSERT INTO integration_checks VALUES(?1,?2,?3)",params![result["id"].as_str().unwrap(),profile_id,result.to_string()]).map_err(|e|e.to_string())?;Ok(result)
  },
  Action::Export{expected_digest,..}=>{let(j,files,digest)=src.ok_or_else(||source_error.unwrap_or("制作Jobなし".into()))?;if digest!=&expected_digest{return Err("完成品のhashが変わっています。再確認してください".into())}Ok(json!({"delivery":export(e,db,j,files.clone(),digest)?}))},
  Action::PrepareExternal{expected_digest,plan,..}=>{let(j,_,digest)=src.ok_or_else(||source_error.unwrap_or("制作Jobなし".into()))?;if digest!=&expected_digest{return Err("完成品のhashが変わっています。再確認してください".into())}let plan=bind(db,plan)?;Ok(json!({"delivery":external(db,j,digest,plan)?}))},
  Action::VerifyExport{delivery_id}=>{let mut r=load_delivery(db,&delivery_id)?;let check=verify_package(e,&r);r.history.push(json!({"at":now(),"operation":"RECONCILE_EXPORT","result":if check.is_ok(){"PASS"}else{"FAILED"},"error":check.as_ref().err().map(|s|redact(s))}));if check.is_ok(){r.status="EXPORTED".into()}save_delivery(db,&r)?;check?;Ok(json!({"delivery":r}))},
 }
}
// Kept separate so the settings/ledger read never recurses into the Engine mutex.
fn view_with_snapshot(e:&Engine,db:&Connection,job_id:Option<&str>,src:Option<&(Job,Vec<PackageFile>,String)>,error:Option<String>,snapshot:&crate::model::Snapshot)->Result<Value,String>{
 let profiles=records(db,"SELECT json FROM integration_profiles ORDER BY rowid")?;let checks=records(db,"SELECT json FROM integration_checks ORDER BY rowid DESC")?;let mut deliveries=records(db,"SELECT json FROM delivery_runs ORDER BY rowid DESC")?;
 if let Some(id)=job_id{deliveries.retain(|v|v["jobId"]==id)}
 for v in &mut deliveries{let r:Delivery=serde_json::from_value(v.clone()).map_err(|e|e.to_string())?;v["packageIntegrity"]=json!(if r.plan.goal==Goal::ExportPackage{if verify_package(e,&r).is_ok(){"PASS"}else{"FAILED"}}else{"NOT_APPLICABLE"});}
 let plan=job_id.and_then(|id|snapshot.completion_plans.get(id));let goal=plan.map(|p|p.goal.clone()).unwrap_or_default();
 let reached=src.is_some()&&match goal{Goal::LocalFiles=>true,Goal::ExportPackage=>deliveries.iter().any(|v|v["status"]=="EXPORTED"&&v["packageIntegrity"]=="PASS"&&src.is_some_and(|s|v["sourceDigest"]==s.2)),_=>false};
 Ok(json!({"version":1,"profiles":profiles,"checks":checks,"deliveries":deliveries,"sourceDigest":src.map(|s|&s.2),"sourceError":error,"completionPlan":plan,"goal":goal,"goalReached":reached,"productionComplete":src.is_some(),"connectionStatus":"NOT_TESTED","accountCheck":"NOT_VERIFIED","liveExecution":"NOT_IMPLEMENTED","publication":"NOT_PERFORMED"}))
}
pub fn production_completed(e:&Engine,job_id:&str)->Result<(),String>{
 let s=e.snapshot();let Some(plan)=s.completion_plans.get(job_id) else{return Ok(())};
 if plan.goal==Goal::LocalFiles{return Ok(())}
 let (j,files,digest)=source(e,job_id)?;let d=e.data.lock().map_err(|_|"状態ロックエラー")?;init(&d.store.0)?;
 if plan.goal==Goal::ExportPackage{export(e,&d.store.0,&j,files,&digest)?;}else{external(&d.store.0,&j,&digest,plan.clone())?;}Ok(())
}
