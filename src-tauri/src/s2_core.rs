//! Image-to-HTML core. State is part of the existing SQLite Job, never a second ledger.
//! LOCAL_ONLY ingress is restricted to explicitly marked QA data. Official ingress
//! remains unavailable until the supported CUA execution host is connected.
use crate::{engine::{Engine,artifact,verify_artifact,inspect_image_bytes},model::*,practical::Quality,recovery::{sha,install_exact}};
use serde::{Deserialize,Serialize};
use std::{fs,path::{Path,PathBuf}};

pub const HOST_BLOCKER:&str="OFFICIAL_EDGE_CUA_HOST_UNAVAILABLE: 独立exe向けの正式なCUA呼出契約が未確認です";
#[cfg(test)]thread_local!{pub static FAULT:std::cell::RefCell<Option<&'static str>>=const{std::cell::RefCell::new(None)};}
fn fault(stage:&str)->Result<(),String>{
 #[cfg(test)]{if FAULT.with(|f|f.borrow().as_ref()==Some(&stage)){return Err(format!("LOCAL_ONLY injected crash: {stage}"))}}
 let _=stage;Ok(())
}
pub fn host_status()->serde_json::Value{serde_json::json!({"route":{"family":"edge","type":"extension"},"status":"HOST_CONTRACT_UNCONFIRMED","blockedScope":"CHATGPT_DELEGATION_ONLY","codexOnlyAvailable":true,"connected":false,"ready":false,"reason":HOST_BLOCKER,"officialSend":false,"officialDownload":false,"fallback":null})}
/// Existing isolated Job only; same production Engine, no new Job or AI calls.
pub fn cli(args:&[String])->Result<(),String>{
 if args.len()!=5{return Err("--local-product EXISTING_QA_DATA REQUEST_JSON NEW_REPORT_JSON".into())}
 let data=Path::new(&args[2]);let report=Path::new(&args[4]);if !data.is_absolute()||!data.join(".ai-guild-qa").is_file()||!data.join("guild.sqlite3").is_file()||!report.is_absolute()||report.exists(){return Err("Existing isolated data and new report required".into())}
 let request:serde_json::Value=serde_json::from_slice(&fs::read(&args[3]).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 let b:Identity=serde_json::from_value(request["identity"].clone()).map_err(|e|e.to_string())?;let e=Engine::open(data,None)?;
 let field=|name:&str|request[name].as_str().map(String::from).ok_or_else(||format!("Missing {name}"));
 let result=match field("action")?.as_str(){
  "receive"=>e.receive_local_material(b,field("artifactId")?,PathBuf::from(field("source")?)),
  "review"=>e.review_local_material(b,field("artifactId")?,PathBuf::from(field("review")?)),
  "integrate"=>e.integrate_local_product(b,fs::read_to_string(field("html")?).map_err(|e|e.to_string())?),
  "recover"=>e.recover_local_core(b),
  "finish"=>e.finish_local_product(b,PathBuf::from(field("review")?),PathBuf::from(field("visualEvidence")?)),
  _=>Err("Unsupported local action".into())};
 install_exact(report,&serde_json::to_vec_pretty(&serde_json::json!({"classification":"LOCAL_ONLY","at":now(),"result":if result.is_ok(){"PASS"}else{"FAILED"},"error":result.as_ref().err(),"newJobs":0,"inference":0,"send":0,"generation":0,"download":0,"snapshot":e.snapshot()})).unwrap())?;result
}
#[derive(Clone,Serialize,Deserialize,Debug,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Identity{pub job_id:String,pub task_id:String,pub attempt:u32}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ExpectedImage{pub artifact_id:String,pub purpose:String,pub width:u32,pub height:u32,pub source_sha256:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Review{pub subject_sha256:String,pub verdict:String,pub reviewer:String,pub rationale:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Receipt{pub identity:Identity,pub artifact_id:String,pub relative_path:String,pub source:String,pub provenance:String,pub format:String,pub width:u32,pub height:u32,pub bytes:u64,pub sha256:String,pub received_at:String,pub content_review:Option<Review>,pub review_artifact:Option<Artifact>}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct PendingReceipt{pub artifact_id:String,pub source:String,pub relative_path:String,pub sha256:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Failure{pub at:String,pub stage:String,pub symptom:String,pub wait_ms:Option<u64>,pub impact:String,pub recovery:String,pub retries:u32,pub resolved:bool}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Core{pub version:u32,pub identity:Identity,pub scope:String,pub stage:String,pub expected:Vec<ExpectedImage>,pub receipts:Vec<Receipt>,pub pending:Option<PendingReceipt>,pub product_source:Option<String>,pub display_review:Option<Review>,pub display_evidence:Option<Artifact>,pub failures:Vec<Failure>}
impl Core{
 pub fn local(identity:Identity,expected:Vec<ExpectedImage>)->Result<Self,String>{
  if identity.attempt==0||!safe_id(&identity.task_id)||expected.is_empty()||expected.len()>8{return Err("Invalid image contract".into())}
  let mut ids=std::collections::HashSet::new();for x in &expected{if !safe_id(&x.artifact_id)||!ids.insert(&x.artifact_id)||x.purpose.trim().is_empty()||x.width==0||x.height==0||x.width>8192||x.height>8192||!valid_sha(&x.source_sha256){return Err("Invalid expected image".into())}}
  Ok(Self{version:1,identity,scope:"LOCAL_ONLY".into(),stage:"PREPARED".into(),expected,receipts:vec![],pending:None,product_source:None,display_review:None,display_evidence:None,failures:vec![]})
 }
}
fn safe_id(s:&str)->bool{!s.is_empty()&&s.len()<100&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')}
fn valid_sha(s:&str)->bool{s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit()&& !b.is_ascii_uppercase())}
fn install(root:&Path,relative:&str,bytes:&[u8])->Result<PathBuf,String>{
 let p=root.join(relative);if p.exists(){contained_file(root,relative)?;}else if reparse(&fs::symlink_metadata(root).map_err(|e|e.to_string())?){return Err("Job root reparse point".into())}
 install_exact(&p,bytes)?;contained_file(root,relative)
}
fn reparse(m:&fs::Metadata)->bool{
 #[cfg(windows)]{use std::os::windows::fs::MetadataExt;m.file_attributes()&0x400!=0}
 #[cfg(not(windows))]{m.file_type().is_symlink()}
}
/// No URL decoding, alternate streams, drive, query, traversal, symlink or junction.
pub fn contained_file(root:&Path,relative:&str)->Result<PathBuf,String>{
 if relative.is_empty()||relative.contains(['\\',':','%','?','#'])||relative.starts_with('/')||relative.split('/').any(|c|c.is_empty()||c=="."||c==".."||c.ends_with('.')||!c.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_.".contains(&b))){return Err("不正な相対画像path".into())}
 let root_meta=fs::symlink_metadata(root).map_err(|e|e.to_string())?;if reparse(&root_meta){return Err("Job root is a reparse point".into())}
 let canonical=root.canonicalize().map_err(|e|e.to_string())?;let mut p=root.to_path_buf();
 for c in relative.split('/'){p.push(c);let m=fs::symlink_metadata(&p).map_err(|_|format!("登録file欠落: {relative}"))?;if reparse(&m){return Err("symlink/reparse point拒否".into())}}
 let resolved=p.canonicalize().map_err(|e|e.to_string())?;if !resolved.starts_with(&canonical)||!resolved.is_file(){return Err("Job範囲外/非file".into())}Ok(resolved)
}
fn core<'a>(j:&'a Job,b:&Identity)->Result<&'a Core,String>{
 let c=j.collaboration.as_ref().ok_or("画像付き制作契約がありません")?;
 if c.version!=1||j.workflow_id!="s2-image-local-v1"||j.workflow.nodes.len()!=4||j.nodes.len()!=4||![&b.task_id,"writing","qa","delivery"].iter().all(|id|j.nodes.contains_key(*id)){return Err("wrong image pipeline contract".into())}
 Core::local(c.identity.clone(),c.expected.clone())?;
 if c.identity!=*b||j.job_id!=b.job_id||j.nodes.get(&b.task_id).is_none_or(|r|r.attempt!=b.attempt){return Err("wrong Job/Task/attempt".into())}Ok(c)
}
fn local_only(s:&Snapshot,b:&Identity)->Result<(),String>{
 let j=s.job.as_ref().ok_or("Job missing")?;let c=core(j,b)?;
 if c.scope!="LOCAL_ONLY"||!Path::new(&s.data_path).join(".ai-guild-qa").is_file(){return Err(HOST_BLOCKER.into())}
 let data=Path::new(&s.data_path).canonicalize().map_err(|e|e.to_string())?;let workspace=Path::new(&j.workspace).canonicalize().map_err(|e|e.to_string())?;if workspace==data||!workspace.starts_with(&data){return Err("LOCAL_ONLY workspace must be inside QA data".into())}
 if j.mode!="local_only"||j.thread_id.is_some()||j.turn_id.is_some(){return Err("LOCAL_ONLY isolation mismatch".into())}Ok(())
}
fn read_review(path:&Path,subject:&str)->Result<Review,String>{
 let review:Review=serde_json::from_slice(&fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 if review.subject_sha256!=subject||review.verdict!="PASS"||review.reviewer.trim().is_empty()||review.rationale.trim().is_empty(){return Err("hashに紐づく内容QAが未合格です".into())}Ok(review)
}
fn check_receipt(j:&Job,c:&Core,r:&Receipt,require_review:bool)->Result<(),String>{
 let spec=c.expected.iter().find(|x|x.artifact_id==r.artifact_id).ok_or("未登録素材")?;
 if r.identity!=c.identity||r.sha256!=spec.source_sha256||r.format!="PNG"||r.provenance!="LOCAL_ONLY_FIXTURE_NOT_CHATGPT"{return Err("素材の所属/provenance不一致".into())}
 let p=contained_file(Path::new(&j.workspace),&r.relative_path)?;let bytes=fs::read(&p).map_err(|e|e.to_string())?;
 if sha(&bytes)!=r.sha256||bytes.len() as u64!=r.bytes||!bytes.starts_with(b"\x89PNG\r\n\x1a\n"){return Err("素材hash/format不一致".into())}
 if inspect_image_bytes(&bytes)?!=(spec.width,spec.height)||(r.width,r.height)!=(spec.width,spec.height){return Err("素材寸法不一致".into())}
 let matches=j.artifacts.iter().filter(|a|a.path==p.to_string_lossy().trim_start_matches("\\\\?\\")).collect::<Vec<_>>();
 if matches.len()!=1||matches[0].node_id!=c.identity.task_id||matches[0].sha256!=r.sha256||matches[0].kind!="local_image"{return Err("画像manifest不一致".into())}verify_artifact(matches[0],Path::new(&j.workspace))?;
 if require_review{let evidence=r.review_artifact.as_ref().ok_or("素材C evidence missing")?;verify_artifact(evidence,Path::new(&j.workspace))?;let review=read_review(Path::new(&evidence.path),&r.sha256)?;let saved=r.content_review.as_ref().ok_or("素材C未合格")?;if serde_json::to_value(review).unwrap()!=serde_json::to_value(saved).unwrap(){return Err("素材C review drift".into())}}
 Ok(())
}
pub fn verify_materials(j:&Job)->Result<Vec<String>,String>{
 let c=j.collaboration.as_ref().ok_or("素材契約なし")?;core(j,&c.identity)?;
 if c.scope!="LOCAL_ONLY"||j.mode!="local_only"{return Err(HOST_BLOCKER.into())}
 if c.receipts.len()!=c.expected.len(){return Err("必須画像の数量不足/重複".into())}
 let mut seen=std::collections::HashSet::new();let mut paths=vec![];
 for r in &c.receipts{if !seen.insert(&r.artifact_id){return Err("素材二重受領".into())}check_receipt(j,c,r,true)?;paths.push(r.relative_path.clone())}Ok(paths)
}
pub fn inspect_product(j:&Job,raw:&str)->Result<Quality,String>{
 let paths=verify_materials(j)?;let doc=crate::html_policy::inspect_with_images(raw,&paths)?;
 for p in &paths{if !doc.images.contains(p){return Err("完成HTMLが必須素材Cを参照していません".into())}}
 let p=j.practical.as_ref().ok_or("完成品の原文契約なし")?;let mut q=crate::practical::inspect_document(raw,&doc,p.plan.as_ref().ok_or("完成品Planなし")?,&p.original_instruction)?;
 q.checks.push("same Job/Task/attempt PNG + material C + relative references; print N/A".into());
 let c=j.collaboration.as_ref().unwrap();
 if let(Some(review),Some(evidence))=(&c.display_review,&c.display_evidence){verify_artifact(evidence,Path::new(&j.workspace))?;if review.subject_sha256!=q.artifact_sha256||review.verdict!="PASS"{return Err("完成品D表示QAのhash不一致".into())}let records=j.artifacts.iter().filter(|a|a.kind=="local_display_review").collect::<Vec<_>>();if records.len()!=1{return Err("完成品D review evidence missing/duplicate".into())}let actual=read_review(Path::new(&records[0].path),&q.artifact_sha256)?;if serde_json::to_value(actual).unwrap()!=serde_json::to_value(review).unwrap(){return Err("完成品D review drift".into())}q.display_status="PASS_LOCAL_ONLY".into()}
 Ok(q)
}
pub fn verify_product(j:&Job)->Result<Quality,String>{
 let html=j.artifacts.iter().filter(|a|a.name=="event.html").collect::<Vec<_>>();if html.len()!=1{return Err("完成HTML missing/duplicate".into())}
 for a in &j.artifacts{verify_artifact(a,Path::new(&j.workspace))?;}
 let p=contained_file(Path::new(&j.workspace),"event.html")?;if p.to_string_lossy().trim_start_matches("\\\\?\\")!=html[0].path{return Err("完成HTML path不一致".into())}
 let raw=fs::read_to_string(p).map_err(|e|e.to_string())?;let q=inspect_product(j,&raw)?;
 if q.artifact_sha256!=html[0].sha256{return Err("完成HTML manifest不一致".into())}Ok(q)
}
impl Engine{
 fn core_failure(&self,b:&Identity,stage:&str,error:&str){let _=self.update(|s|{let j=s.job.as_mut().ok_or("Job missing")?;core(j,b)?;if j.status=="completed"{return Err("Completed Job immutable".into())}j.collaboration.as_mut().unwrap().failures.push(Failure{at:now(),stage:stage.into(),symptom:error.into(),wait_ms:None,impact:"次工程は未許可。元file・Task・attempt保持".into(),recovery:"同じTaskの既存fileとjournalを再照合".into(),retries:0,resolved:false});s.event(Some(&b.task_id),"material_recovery_required","disconnected",error,None,vec![]);Ok(())});}
 pub fn receive_local_material(&self,b:Identity,artifact_id:String,source:PathBuf)->Result<(),String>{
  let _gate=self.response_gate.lock().map_err(|_|"core lock")?;
  let result=(||{
   let s=self.snapshot();local_only(&s,&b)?;let j=s.job.as_ref().unwrap();let c=core(j,&b)?;
   if j.status=="completed"{return Err("Completed Job immutable".into())}
   let spec=c.expected.iter().find(|x|x.artifact_id==artifact_id).ok_or("wrong artifact")?;
   if !source.is_absolute(){return Err("source absolute path required".into())}
   let relative=source.strip_prefix(Path::new(&s.data_path)).map_err(|_|"LOCAL_ONLY source must be in QA root")?.to_string_lossy().replace('\\',"/");
   let original=source.to_string_lossy().trim_start_matches("\\\\?\\").to_string();
   let read_path=if let Some(p)=c.pending.as_ref().filter(|p|p.artifact_id==artifact_id&&p.source==original&&Path::new(&j.workspace).join(&p.relative_path).exists()){contained_file(Path::new(&j.workspace),&p.relative_path)?}else{contained_file(Path::new(&s.data_path),&relative)?};
   let bytes=fs::read(&read_path).map_err(|e|e.to_string())?;
   if sha(&bytes)!=spec.source_sha256||!bytes.starts_with(b"\x89PNG\r\n\x1a\n")||inspect_image_bytes(&bytes)?!=(spec.width,spec.height){return Err("元画像のhash/PNG完全decode/寸法不一致".into())}
   if let Some(r)=c.receipts.iter().find(|r|r.artifact_id==artifact_id){check_receipt(j,c,r,false)?;return Ok(())}
   let relative_path=format!("assets/{}-{}-{}.png",b.task_id,b.attempt,artifact_id);
   let pending=PendingReceipt{artifact_id:artifact_id.clone(),source:source.to_string_lossy().trim_start_matches("\\\\?\\").into(),relative_path:relative_path.clone(),sha256:sha(&bytes)};
   if let Some(old)=&c.pending{if serde_json::to_value(old).unwrap()!=serde_json::to_value(&pending).unwrap(){return Err("別の受領intentが未完了です".into())}}
   self.update(|s|{let j=s.job.as_mut().unwrap();core(j,&b)?;if j.status=="disconnected"{j.status="running".into()}if j.nodes[&b.task_id].status!="completed"{j.nodes.get_mut(&b.task_id).unwrap().status="running".into()}let c=j.collaboration.as_mut().unwrap();c.pending=Some(pending);c.stage="RECEIVING".into();Ok(())})?;
   fault("receipt.after_intent")?;
   // Never follow a pre-existing assets reparse point, even inside the Job.
   let assets=Path::new(&j.workspace).join("assets");if assets.exists(){if reparse(&fs::symlink_metadata(&assets).map_err(|e|e.to_string())?){return Err("assets reparse point".into())}}else{fs::create_dir(&assets).map_err(|e|e.to_string())?}
   let target=Path::new(&j.workspace).join(&relative_path);if target.exists(){contained_file(Path::new(&j.workspace),&relative_path)?;}
   install(Path::new(&j.workspace),&relative_path,&bytes)?;let a=artifact(&target,&b.task_id,"local_image")?;
   fault("receipt.after_file")?;
   self.update(|s|{let j=s.job.as_mut().unwrap();core(j,&b)?;if j.artifacts.iter().any(|x|x.path==a.path){return Err("SQLite/file receipt conflict".into())}j.artifacts.push(a.clone());let c=j.collaboration.as_mut().unwrap();c.receipts.push(Receipt{identity:b.clone(),artifact_id:artifact_id.clone(),relative_path,source:source.to_string_lossy().into(),provenance:"LOCAL_ONLY_FIXTURE_NOT_CHATGPT".into(),format:"PNG".into(),width:spec.width,height:spec.height,bytes:a.bytes,sha256:a.sha256,received_at:now(),content_review:None,review_artifact:None});c.pending=None;c.stage="MATERIAL_RECEIVED".into();s.event(Some(&b.task_id),"material_received","running","LOCAL_ONLY画像の実bytes・完全decode・hashを受領。実ChatGPT回収ではありません。",None,vec![a.path]);Ok(())})
  })();if let Err(e)=&result{self.core_failure(&b,"RECEIPT",e)}result
 }
 pub fn review_local_material(&self,b:Identity,id:String,review_path:PathBuf)->Result<(),String>{
  let result=self.review_local_material_inner(b.clone(),id,review_path);if let Err(e)=&result{self.core_failure(&b,"MATERIAL_C",e)}result
 }
 fn review_local_material_inner(&self,b:Identity,id:String,review_path:PathBuf)->Result<(),String>{
  let _gate=self.response_gate.lock().map_err(|_|"core lock")?;let s=self.snapshot();local_only(&s,&b)?;let j=s.job.as_ref().unwrap();if j.status=="completed"{return Err("Completed Job immutable".into())}let c=core(j,&b)?;let r=c.receipts.iter().find(|r|r.artifact_id==id).ok_or("素材なし")?;check_receipt(j,c,r,false)?;let review=read_review(&review_path,&r.sha256)?;
  let target=install(Path::new(&j.workspace),&format!("material-{id}-review.json"),&serde_json::to_vec_pretty(&review).unwrap())?;let a=artifact(&target,&b.task_id,"local_material_review")?;
  self.update(|s|{let j=s.job.as_mut().unwrap();core(j,&b)?;if j.status=="disconnected"{j.status="running".into()}if j.nodes[&b.task_id].status=="disconnected"{j.nodes.get_mut(&b.task_id).unwrap().status="running".into()}Ok(())})?;
  self.update(|s|{let j=s.job.as_mut().unwrap();core(j,&b)?;let c=j.collaboration.as_mut().unwrap();let r=c.receipts.iter_mut().find(|r|r.artifact_id==id).unwrap();r.content_review=Some(review);r.review_artifact=Some(a.clone());if !j.artifacts.iter().any(|x|x.path==a.path){j.artifacts.push(a)}if verify_materials(j).is_ok(){j.collaboration.as_mut().unwrap().stage="MATERIAL_C_PASS".into();j.nodes.get_mut(&b.task_id).unwrap().status="completed".into();}s.event(Some(&b.task_id),"material_quality_checked","running","素材Cの内容QAと元fileのhashを照合（LOCAL_ONLY）。",None,vec![]);Ok(())})
 }
 pub fn integrate_local_product(&self,b:Identity,raw:String)->Result<(),String>{
  let _gate=self.response_gate.lock().map_err(|_|"core lock")?;
  let result=(||{let s=self.snapshot();local_only(&s,&b)?;let j=s.job.as_ref().unwrap();core(j,&b)?;inspect_product(j,&raw)?;
   if let Some(old)=&j.collaboration.as_ref().unwrap().product_source{if old!=&raw{return Err("既存HTML原bytesを変更しません".into())}}
   if j.status=="completed"{verify_product(j)?;return Ok(())}
   self.update(|s|{let j=s.job.as_mut().unwrap();core(j,&b)?;if j.status=="disconnected"{j.status="running".into()}if j.nodes["writing"].status!="completed"{j.nodes.get_mut("writing").unwrap().status="running".into()}let c=j.collaboration.as_mut().unwrap();c.stage="INTEGRATING".into();c.product_source=Some(raw.clone());s.event(Some("writing"),"integration_started","running","承認済み素材Cを完成HTMLへ統合（LOCAL_ONLY）。",None,vec![]);Ok(())})?;
   fault("integration.after_intent")?;
   let target=install(Path::new(&j.workspace),"event.html",raw.as_bytes())?;let a=artifact(&target,"writing","local_html")?;
   fault("integration.after_file")?;
   self.update(|s|{let j=s.job.as_mut().unwrap();core(j,&b)?;if let Some(old)=j.artifacts.iter().find(|x|x.name=="event.html"){if old.sha256!=a.sha256||old.path!=a.path{return Err("SQLite/HTML conflict".into())}}else{j.artifacts.push(a)}let q=verify_product(j)?;j.quality=Some(q);j.collaboration.as_mut().unwrap().stage="D_MACHINE_PASS_DISPLAY_PENDING".into();j.nodes.get_mut("writing").ok_or("writing Task missing")?.status="completed".into();if j.nodes["qa"].status!="completed"{j.nodes.get_mut("qa").unwrap().status="running".into()}s.event(Some("qa"),"product_quality_checked","running","画像参照・必須文言・保存hashはPASS。表示QAは未完了。印刷N/A。",None,vec![]);Ok(())})
  })();if let Err(e)=&result{self.core_failure(&b,"INTEGRATION",e)}result
 }
 pub fn finish_local_product(&self,b:Identity,review_path:PathBuf,visual_evidence:PathBuf)->Result<(),String>{
  let result=self.finish_local_product_inner(b.clone(),review_path,visual_evidence);if let Err(e)=&result{self.core_failure(&b,"FINISH",e)}result
 }
 fn finish_local_product_inner(&self,b:Identity,review_path:PathBuf,visual_evidence:PathBuf)->Result<(),String>{
  let _gate=self.response_gate.lock().map_err(|_|"core lock")?;
  let s=self.snapshot();local_only(&s,&b)?;let j=s.job.as_ref().unwrap();let q=verify_product(j)?;let review=read_review(&review_path,&q.artifact_sha256)?;
  if j.status=="completed"{return Ok(())}
  let evidence=fs::read(&visual_evidence).map_err(|e|e.to_string())?;inspect_image_bytes(&evidence)?;
  if !evidence.starts_with(b"\x89PNG\r\n\x1a\n"){return Err("表示証拠はPNGが必要です".into())}
  self.update(|s|{let j=s.job.as_mut().unwrap();core(j,&b)?;if j.status=="disconnected"{j.status="running".into()}if j.nodes["qa"].status=="disconnected"{j.nodes.get_mut("qa").unwrap().status="running".into()}Ok(())})?;
  let target=install(Path::new(&j.workspace),"display-evidence.png",&evidence)?;let a=artifact(&target,"qa","local_display_evidence")?;
  let review_target=install(Path::new(&j.workspace),"display-review.json",&serde_json::to_vec_pretty(&review).unwrap())?;let review_artifact=artifact(&review_target,"qa","local_display_review")?;
  self.update(|s|{let j=s.job.as_mut().unwrap();core(j,&b)?;j.collaboration.as_mut().unwrap().display_review=Some(review);j.collaboration.as_mut().unwrap().display_evidence=Some(a.clone());for file in [a,review_artifact]{if !j.artifacts.iter().any(|x|x.path==file.path){j.artifacts.push(file)}}j.quality=Some(verify_product(j)?);j.nodes.get_mut("qa").ok_or("qa Task missing")?.status="completed".into();Ok(())})?;
  // Existing save gate and manifest are reused. A crash here is resumed from the
  // same product_source/review, without creating another Task or image.
  self.save_local_manifest(&b)
 }
 fn save_local_manifest(&self,b:&Identity)->Result<(),String>{
  let s=self.snapshot();let j=s.job.as_ref().unwrap();let q=verify_product(j)?;if q.display_status!="PASS_LOCAL_ONLY"{return Err("完成品D表示未検証".into())}
  let artifacts=j.artifacts.iter().filter(|a|a.kind!="local_manifest").collect::<Vec<_>>();let value=serde_json::json!({"jobId":j.job_id,"identity":b,"classification":"LOCAL_ONLY","officialSend":false,"officialDownload":false,"print":"N/A","artifacts":artifacts,"htmlSha256":q.artifact_sha256});
  self.update(|s|{let j=s.job.as_mut().unwrap();core(j,b)?;if j.nodes["delivery"].status!="completed"{j.nodes.get_mut("delivery").unwrap().status="running".into()}Ok(())})?;
  let target=install(Path::new(&j.workspace),"manifest.json",&serde_json::to_vec_pretty(&value).unwrap())?;let a=artifact(&target,"delivery","local_manifest")?;
  fault("manifest.after_file")?;
  self.update(|s|{let j=s.job.as_mut().unwrap();core(j,b)?;verify_product(j)?;if !j.artifacts.iter().any(|x|x.path==a.path){j.artifacts.push(a)}j.collaboration.as_mut().unwrap().stage="DONE_LOCAL_ONLY".into();for f in &mut j.collaboration.as_mut().unwrap().failures{f.resolved=true}for r in j.nodes.values_mut(){r.status="completed".into();r.ended_at=Some(now())}j.status="completed".into();j.ended_at=Some(now());j.active_node_id=None;s.event(Some("delivery"),"local_product_saved","completed","LOCAL_ONLY完成品を保存。実AI共同制作E2Eは未実行です。",None,vec![]);Ok(())})
 }
 pub fn recover_local_core(&self,b:Identity)->Result<(),String>{
  let result=self.recover_local_core_inner(b.clone());if let Err(e)=&result{self.core_failure(&b,"RECOVERY",e)}result
 }
 fn recover_local_core_inner(&self,b:Identity)->Result<(),String>{
  let s=self.snapshot();local_only(&s,&b)?;let j=s.job.as_ref().unwrap();let c=core(j,&b)?.clone();
  if j.status=="completed"{verify_product(j)?;return Ok(())}
  if let Some(p)=c.pending{self.receive_local_material(b.clone(),p.artifact_id,PathBuf::from(p.source))?;}
  let s=self.snapshot();let j=s.job.as_ref().unwrap();for r in &j.collaboration.as_ref().unwrap().receipts{check_receipt(j,j.collaboration.as_ref().unwrap(),r,false)?}
  if let Some(raw)=c.product_source{self.integrate_local_product(b.clone(),raw)?;if c.display_review.is_some(){self.save_local_manifest(&b)?;}}
  Ok(())
 }
}
