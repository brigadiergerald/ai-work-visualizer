//! Optional, read-only decision evidence. Formal outputs are the only narrative source.
//! No inference, extra turns, Job migration, approval action or renderer file access.
use crate::{model::*,presentation::TaskExecutionBinding};
use serde::Deserialize;
use serde_json::{json,Value};
use std::{fs,path::Path};

pub fn enabled(j:&Job)->bool{j.workflow_id==crate::sns_team::DECISIONS_ID}
/// Keep established production validators strict. Only the new workflow may carry
/// the optional notes; malformed/missing notes cannot stop an otherwise valid Job.
pub fn production_payload(j:&Job,raw:&str)->String{
 if !enabled(j){return raw.into()}
 let Ok(mut v)=serde_json::from_str::<Value>(raw)else{return raw.into()};
 if let Some(o)=v.as_object_mut(){o.remove("decisionNotes");}v.to_string()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Note{summary:String,reason:String,details:Vec<String>}
fn note(v:&Value)->Option<Value>{
 let n:Note=serde_json::from_value(v.clone()).ok()?;
 if n.summary.trim().is_empty()||n.reason.trim().is_empty()||n.summary.chars().count()>600||n.reason.chars().count()>600||n.details.len()>12||n.details.iter().any(|s|s.trim().is_empty()||s.chars().count()>1200){return None}
 Some(json!({"summary":n.summary,"reason":n.reason,"details":n.details}))
}
/// Called only AFTER verify_product succeeds, at the actual quality stage.
/// Criteria are codes; observations are measured values, not claims of semantic QA.
pub fn inspection(j:&Job)->Value{
 let plan=j.artifacts.iter().find(|a|a.name=="plan.json").and_then(|a|fs::read(&a.path).ok()).and_then(|b|serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
 let post=j.artifacts.iter().find(|a|a.kind=="product"&&a.name=="post.txt");
 let characters=post.and_then(|a|fs::read_to_string(&a.path).ok()).map(|s|s.chars().count());
 json!({"version":1,"status":"MACHINE_CHECKS_PASSED","basis":"HOST_VERIFY_PRODUCT","checks":[
  {"criterion":"EXACT_REQUIRED_COPY","result":"PASS","observed":crate::codex_only::exact_copy(&j.instruction).len()+plan["files"].as_array().map(|fs|fs.iter().map(|f|f["requiredText"].as_array().map(Vec::len).unwrap_or(0)).sum::<usize>()).unwrap_or(0)},
  {"criterion":"PLANNED_FILES_AND_BUNDLE","result":"PASS","observed":j.artifacts.iter().filter(|a|a.kind=="product").count()},
  {"criterion":"REGISTERED_BYTES_SHA256","result":"PASS","observed":j.artifacts.len()},
  {"criterion":"CHARACTER_COUNT","result":"MEASURED_ONLY","observed":characters}
 ],"unverified":["SEMANTIC_INSTRUCTION_ALIGNMENT","MEDIA_LENGTH_COMPLIANCE","PROHIBITED_CONTENT","FACTS_EXPERIENCES_NUMBERS","USEFULNESS_AND_DISPLAY"],"nextStageBasis":"VERIFIED_LOCAL_FILES_ONLY","humanContentApproval":"NOT_PERFORMED","publication":"NOT_PERFORMED"})
}
fn source<'a>(j:&'a Job,s:&Snapshot,b:&TaskExecutionBinding,name:&str)->Option<(&'a Artifact,String)>{
 let event=s.events.iter().find(|e|e.job_id==j.job_id&&e.node_id.as_deref()==Some(&b.node_id)&&e.event_type=="node_completed"&&s.presentation_evidence.get(&e.event_id).and_then(|p|p.task_id.as_ref())==Some(&b.task_id))?;
 let files=j.artifacts.iter().filter(|a|a.node_id==b.node_id&&a.name==name&&event.artifact_paths.contains(&a.path)).collect::<Vec<_>>();
 if files.len()!=1{return None}Some((files[0],event.timestamp.clone()))
}
pub fn view(j:&Job,s:&Snapshot)->Value{
 let records=[("theme","plan","plan.json"),("direction","plan","plan.json"),("writing","writing","bundle.json"),("inspection","qa","quality.json")].into_iter().map(|(kind,node,name)|{
  let run=j.nodes.get(node);let binding=s.task_executions.iter().find(|b|b.job_id==j.job_id&&b.node_id==node&&run.is_some_and(|r|r.attempt==b.attempt));
  let mut v=json!({"kind":kind,"jobId":j.job_id,"nodeId":node,"taskId":binding.map(|b|&b.task_id),"attempt":binding.map(|b|b.attempt),"roleId":binding.and_then(|b|b.role_id.as_ref()),"stageError":run.and_then(|r|r.error.as_ref()),"sourceFields":null,"stageState":run.map(|r|r.status.as_str()).unwrap_or("unknown"),"status":"NO_RECORD","recordedAt":null,"source":null,"note":null,"inspection":null});
  if let Some((a,at))=binding.and_then(|b|source(j,s,b,name)){
   // Parse exactly the bytes checked below, never a second unchecked file read.
   let bytes=crate::engine::verify_artifact(a,Path::new(&j.workspace)).ok().and_then(|_|fs::read(&a.path).ok()).filter(|b|b.len() as u64==a.bytes&&crate::purposes::sha256(b)==a.sha256);
   let ok=bytes.is_some();
   v["source"]=json!({"name":a.name,"path":a.path,"sha256":a.sha256,"bytes":a.bytes,"integrity":if ok{"verified"}else{"failed"}});v["recordedAt"]=json!(at);
   if !ok{v["status"]=json!("SOURCE_FAILED");return v}
   let raw=bytes.and_then(|b|serde_json::from_slice::<Value>(&b).ok());
   if node=="plan"{if let Some(raw)=&raw{v["sourceFields"]=json!({"requirements":raw["requirements"],"productionConstraints":raw["productionConstraints"]});}}
   if enabled(j){if let Some(raw)=raw{
    if kind=="inspection"{
     if raw["decisionInspection"]["version"]==1&&raw["decisionInspection"]["basis"]=="HOST_VERIFY_PRODUCT"{
      v["inspection"]=raw["decisionInspection"].clone();v["status"]=json!("RECORDED");
     }
    }else if let Some(n)=note(&raw["decisionNotes"][kind]){v["note"]=n;v["status"]=json!("RECORDED");}
    else if !raw["decisionNotes"][kind].is_null(){v["status"]=json!("INVALID_RECORD");}
   }}
  }
  // A failed/UNKNOWN task never becomes completed because a record exists.
  v
 }).collect::<Vec<_>>();
 json!({"version":1,"jobId":j.job_id,"records":records,"humanContentApproval":if s.approvals.iter().any(|a|a.job_id==j.job_id&&a.kind=="content_review"&&!a.resolved){"PENDING"}else if j.artifacts.iter().any(|a|a.name=="review.json"&&a.kind=="review"&&crate::engine::verify_artifact(a,Path::new(&j.workspace)).is_ok()){ "RECORDED_SEPARATELY" }else{"NOT_PERFORMED"},"publication":"NOT_PERFORMED"})
}

