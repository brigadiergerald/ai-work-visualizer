//! Read-only Event/Role v1. Evidence is captured by the existing Engine, never by skins.
use crate::model::{Artifact, Event, Job, Node, Snapshot};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, path::Path};

pub const VERSION: u32 = 1;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleDefinition {pub id: String, pub purpose_id: String, pub label_key: String, pub actor: String}
pub fn roles() -> Vec<RoleDefinition> {
 [("planner","codex"),("writer","codex"),("verifier","host"),("reviewer","human"),("archivist","host")].into_iter().map(|(id,actor)|RoleDefinition{id:id.into(),purpose_id:crate::purposes::SOCIAL.into(),label_key:format!("roles.social-text.{id}"),actor:actor.into()}).collect()
}
pub fn roles_for(j:&Job)->Vec<RoleDefinition>{
 if crate::sns_team::applies(j){return ["researcher","director","creator","inspector"].into_iter().map(|id|RoleDefinition{id:id.into(),purpose_id:crate::purposes::SOCIAL.into(),label_key:format!("roles.social-text.{id}"),actor:if id=="inspector"{"host"}else{"codex"}.into()}).collect()}
 if crate::sns::applies(j){roles()}else{vec![]}
}
fn role(j: &Job, n: &Node) -> Option<String> {
 if crate::sns_team::applies(j){return match(n.node_id.as_str(),n.executor.as_str()) {("research","codex")=>Some("researcher"),("plan","codex")=>Some("director"),("writing","codex")=>Some("creator"),("qa","quality")|("delivery","save")=>Some("inspector"),_=>None}.map(String::from)}
 if !crate::sns::applies(j) {return None}
 match (n.node_id.as_str(), n.executor.as_str()) {("plan","codex")=>Some("planner"),("writing","codex")=>Some("writer"),("qa","quality")=>Some("verifier"),("sns_review","review")=>Some("reviewer"),("delivery","save")=>Some("archivist"),_=>None}.map(String::from)
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskExecutionBinding {
 pub job_id: String, pub task_id: String, pub node_id: String, pub attempt: u32,
 pub role_id: Option<String>, pub thread_id: Option<String>, pub turn_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileProof {pub path:String, pub node_id:String, pub sha256:String, pub bytes:u64}
impl From<&Artifact> for FileProof {fn from(a:&Artifact)->Self{Self{path:a.path.clone(),node_id:a.node_id.clone(),sha256:a.sha256.clone(),bytes:a.bytes}}}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {pub edge_id:String,pub from_task_id:String,pub to_task_id:String,pub source_event_id:String,pub verification:String,pub files:Vec<FileProof>}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventEvidence {pub task_id:Option<String>,pub approval_id:Option<String>,pub receipt:Option<Receipt>}

fn find_job<'a>(s:&'a Snapshot,id:&str)->Result<&'a Job,String>{s.job.iter().chain(s.history.iter()).find(|j|j.job_id==id).ok_or("Presentation Job missing".into())}
pub fn begin_task(s:&mut Snapshot,node:&str)->Result<(),String>{
 let j=s.job.as_ref().ok_or("Task Job missing")?;
 let r=j.nodes.get(node).ok_or("Task node missing")?;let n=j.workflow.nodes.iter().find(|n|n.node_id==node).ok_or("Task workflow node missing")?;
 if r.attempt==0 || s.task_executions.iter().any(|b|b.job_id==j.job_id&&b.node_id==node&&b.attempt==r.attempt){return Err("Task attempt already exists or is zero".into())}
 s.task_executions.push(TaskExecutionBinding{job_id:j.job_id.clone(),task_id:crate::model::id(),node_id:node.into(),attempt:r.attempt,role_id:role(j,n),thread_id:None,turn_id:None});Ok(())
}
fn current<'a>(s:&'a Snapshot,node:&str)->Option<&'a TaskExecutionBinding>{let j=s.job.as_ref()?;let r=j.nodes.get(node)?;s.task_executions.iter().find(|b|b.job_id==j.job_id&&b.node_id==node&&b.attempt==r.attempt)}
pub fn capture_new_executions(before:&Snapshot,next:&mut Snapshot)->Result<(),String>{
 // The existing formal recovery path can start previously unexecuted host
 // stages without run(). Bind only an actually NEW attempt and start Event;
 // an old recovered Codex execution with missing IDs remains unbound.
 let nodes=next.job.as_ref().map(|j|j.nodes.iter().filter(|(node,run)|run.status=="running"&&run.attempt>before.job.as_ref().filter(|old|old.job_id==j.job_id).and_then(|old|old.nodes.get(*node)).map(|r|r.attempt).unwrap_or(0)&&!next.task_executions.iter().any(|b|b.job_id==j.job_id&&b.node_id==**node&&b.attempt==run.attempt)&&next.events.iter().skip(before.events.len()).any(|e|e.job_id==j.job_id&&e.node_id.as_ref()==Some(*node)&&["node_started","recovery_stage_started"].contains(&e.event_type.as_str()))).map(|(id,_)|id.clone()).collect::<Vec<_>>()).unwrap_or_default();
 for node in nodes{begin_task(next,&node)?;let task=current(next,&node).unwrap().task_id.clone();for e in next.events.iter().skip(before.events.len()).filter(|e|e.node_id.as_ref()==Some(&node)){if let Some(p)=next.presentation_evidence.get_mut(&e.event_id){p.task_id=Some(task.clone());}}}Ok(())
}
pub fn bind_formal(s:&mut Snapshot,node:&str,thread:&str,turn:Option<&str>)->Result<(),String>{
 if thread.is_empty()||turn==Some(""){return Err("Empty formal ID".into())}
 let j=s.job.as_ref().ok_or("Formal Job missing")?;
 if j.active_node_id.as_deref()!=Some(node)||j.thread_id.as_deref()!=Some(thread)||turn.is_some_and(|t|j.turn_id.as_deref()!=Some(t)){return Err("Formal Job/node/thread/turn mismatch".into())}
 // Historical execution without a binding stays unbound; never infer past IDs.
 let Some(old)=current(s,node).cloned() else {return Ok(())};
 if old.thread_id.as_deref().is_some_and(|t|t!=thread)||turn.is_some_and(|t|old.turn_id.as_deref().is_some_and(|v|v!=t)){return Err("Formal binding cannot be replaced".into())}
 if let Some(t)=turn{if s.task_executions.iter().any(|b|b.job_id==old.job_id&&b.task_id!=old.task_id&&b.thread_id.as_deref()==Some(thread)&&b.turn_id.as_deref()==Some(t)){return Err("Formal turn already belongs to another task".into())}}
 let b=s.task_executions.iter_mut().find(|b|b.task_id==old.task_id).unwrap();b.thread_id=Some(thread.into());if let Some(t)=turn{b.turn_id=Some(t.into())}Ok(())
}
pub fn capture(s:&mut Snapshot){
 let Some(e)=s.events.last() else{return};let Some(j)=s.job.as_ref()else{return};
 let task=e.node_id.as_deref().and_then(|n|current(s,n)).map(|b|b.task_id.clone());
 let approvals=s.approvals.iter().filter(|a|a.job_id==e.job_id&&Some(&a.node_id)==e.node_id.as_ref()).collect::<Vec<_>>();
 let approval=if e.status=="awaiting_approval" {let pending=approvals.iter().filter(|a|!a.resolved).collect::<Vec<_>>();if pending.len()==1{Some(pending[0].id.clone())}else{None}}else{None};
 if e.job_id==j.job_id{s.presentation_evidence.insert(e.event_id.clone(),EventEvidence{task_id:task,approval_id:approval,receipt:None});}
}
pub fn receive(s:&mut Snapshot,edge_id:&str,files:&[Artifact])->Result<(),String>{
 let j=s.job.as_ref().ok_or("Receipt Job missing")?;let edge=j.workflow.edges.iter().find(|e|e.edge_id==edge_id).ok_or("Receipt edge missing")?;
 let e=s.events.last().filter(|e|e.event_type=="handoff_received"&&e.edge_id.as_deref()==Some(edge_id)&&e.node_id.as_deref()==Some(&edge.to_node_id)).ok_or("Receipt source event mismatch")?;
 // New Job -> old source execution and empty dependencies are NOT a verified handoff.
 let (Some(from),Some(to))=(current(s,&edge.from_node_id),current(s,&edge.to_node_id))else{return Ok(())};
 if files.is_empty(){return Ok(())}
 if j.nodes.get(&edge.from_node_id).is_none_or(|r|r.status!="completed")||files.len()!=j.artifacts.iter().filter(|a|a.node_id==edge.from_node_id).count()||files.iter().any(|a|!j.artifacts.iter().any(|registered|FileProof::from(registered)==FileProof::from(a))){return Err("Receipt source execution/file set mismatch".into())}
 for a in files {if a.node_id!=edge.from_node_id{return Err("Receipt wrong source node".into())}crate::engine::verify_artifact(a,Path::new(&j.workspace))?;}
 let receipt=Receipt{edge_id:edge_id.into(),from_task_id:from.task_id.clone(),to_task_id:to.task_id.clone(),source_event_id:e.event_id.clone(),verification:"FILES_VERIFIED".into(),files:files.iter().map(FileProof::from).collect()};
 s.presentation_evidence.get_mut(&e.event_id).ok_or("Receipt evidence missing")?.receipt=Some(receipt);Ok(())
}

// These mirrors and revision are saved in the SAME Store transaction as Job/Event/context.
pub const SQL:&str="CREATE TABLE IF NOT EXISTS task_execution_binding(task_id TEXT PRIMARY KEY,job_id TEXT NOT NULL,node_id TEXT NOT NULL,attempt INTEGER NOT NULL,json TEXT NOT NULL,UNIQUE(job_id,node_id,attempt)); CREATE TABLE IF NOT EXISTS presentation_event_evidence(event_id TEXT PRIMARY KEY,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS presentation_meta(id INTEGER PRIMARY KEY CHECK(id=1),revision INTEGER NOT NULL); INSERT OR IGNORE INTO presentation_meta VALUES(1,0);";
fn extension(old:&TaskExecutionBinding,new:&TaskExecutionBinding)->bool{let mut b=old.clone();if b.thread_id.is_none(){b.thread_id=new.thread_id.clone()}if b.turn_id.is_none(){b.turn_id=new.turn_id.clone()}b==*new}
pub fn validate(db:&Connection,s:&Snapshot,loading:bool)->Result<(),String>{
 // The snapshot and append-only Event table must describe the same identities.
 // Rowid is a persisted order, never an order inferred from timestamps/messages.
 let mut q=db.prepare("SELECT id,json FROM events ORDER BY rowid").map_err(|e|e.to_string())?;
 let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;let mut event_count=0;
 for row in rows{let(id,raw)=row.map_err(|e|e.to_string())?;let e=s.events.iter().find(|e|e.event_id==id).ok_or("Persisted Event missing from snapshot")?;if serde_json::from_str::<Value>(&raw).map_err(|e|e.to_string())?!=serde_json::to_value(e).map_err(|e|e.to_string())?{return Err("Persisted Event conflict".into())}event_count+=1;}
 if loading&&event_count!=s.events.iter().map(|e|&e.event_id).collect::<HashSet<_>>().len(){return Err("Event mirror count mismatch".into())}
 let mut tasks=HashSet::new();let mut attempts=HashSet::new();let mut turns=HashSet::new();
 for b in &s.task_executions {
  let j=find_job(s,&b.job_id)?;let n=j.workflow.nodes.iter().find(|n|n.node_id==b.node_id).ok_or("Binding node missing")?;
  if b.task_id.is_empty()||b.attempt==0||!tasks.insert(&b.task_id)||!attempts.insert((&b.job_id,&b.node_id,b.attempt))||j.nodes.get(&b.node_id).is_none_or(|r|b.attempt>r.attempt)||b.role_id!=role(j,n)||b.turn_id.is_some()&&b.thread_id.is_none(){return Err("Invalid task/attempt/role binding".into())}
  if let Some(t)=&b.thread_id{if t.is_empty()||j.thread_id.as_ref()!=Some(t){return Err("Binding thread belongs to wrong Job".into())}}
  if let Some(t)=&b.turn_id{if t.is_empty()||!turns.insert((&b.job_id,&b.thread_id,t)){return Err("Duplicate/empty formal turn".into())}}
 }
 for (id,proof) in &s.presentation_evidence {
  let e=s.events.iter().find(|e|&e.event_id==id).ok_or("Evidence event missing")?;
  if let Some(t)=&proof.task_id{if !s.task_executions.iter().any(|b|&b.task_id==t&&b.job_id==e.job_id&&Some(&b.node_id)==e.node_id.as_ref()){return Err("Evidence task/Job/node mismatch".into())}}
  if let Some(a)=&proof.approval_id{if !s.approvals.iter().any(|v|&v.id==a&&v.job_id==e.job_id&&Some(&v.node_id)==e.node_id.as_ref()){return Err("Evidence approval mismatch".into())}}
  if let Some(r)=&proof.receipt {
   let j=find_job(s,&e.job_id)?;let edge=j.workflow.edges.iter().find(|v|v.edge_id==r.edge_id).ok_or("Receipt edge missing")?;
   let from=s.task_executions.iter().find(|b|b.task_id==r.from_task_id&&b.job_id==j.job_id&&b.node_id==edge.from_node_id);
   let to=s.task_executions.iter().find(|b|b.task_id==r.to_task_id&&b.job_id==j.job_id&&b.node_id==edge.to_node_id);
   if e.event_type!="handoff_received"||r.source_event_id!=*id||e.edge_id.as_ref()!=Some(&r.edge_id)||e.node_id.as_ref()!=Some(&edge.to_node_id)||r.verification!="FILES_VERIFIED"||r.files.is_empty()||from.is_none()||to.is_none()||proof.task_id.as_ref()!=Some(&r.to_task_id)||r.files.iter().any(|f|f.node_id!=edge.from_node_id||!j.artifacts.iter().any(|a|FileProof::from(a)==*f)){return Err("Invalid structured handoff receipt".into())}
  }
 }
 let mut q=db.prepare("SELECT task_id,job_id,node_id,attempt,json FROM task_execution_binding").map_err(|e|e.to_string())?;
 let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,u32>(3)?,r.get::<_,String>(4)?))).map_err(|e|e.to_string())?;let mut count=0;
 for row in rows{let(id,job,node,attempt,raw)=row.map_err(|e|e.to_string())?;let old:TaskExecutionBinding=serde_json::from_str(&raw).map_err(|e|e.to_string())?;let new=s.task_executions.iter().find(|b|b.task_id==old.task_id).ok_or("Persisted task binding missing")?;if old.task_id!=id||old.job_id!=job||old.node_id!=node||old.attempt!=attempt||if loading{old!=*new}else{!extension(&old,new)}{return Err("Persisted task binding conflict".into())}count+=1;}
 if loading&&count!=s.task_executions.len(){return Err("Task mirror count mismatch".into())}
 let mut q=db.prepare("SELECT event_id,json FROM presentation_event_evidence").map_err(|e|e.to_string())?;
 let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;let mut count=0;
 for row in rows{let(id,raw)=row.map_err(|e|e.to_string())?;let old:EventEvidence=serde_json::from_str(&raw).map_err(|e|e.to_string())?;if s.presentation_evidence.get(&id)!=Some(&old){return Err("Persisted event evidence conflict".into())}count+=1;}
 if loading&&count!=s.presentation_evidence.len(){return Err("Evidence mirror count mismatch".into())}Ok(())
}
pub fn save(db:&Connection,s:&Snapshot)->Result<(),String>{
 for b in &s.task_executions{db.execute("INSERT INTO task_execution_binding VALUES(?1,?2,?3,?4,?5) ON CONFLICT(task_id) DO UPDATE SET json=excluded.json",params![b.task_id,b.job_id,b.node_id,b.attempt,serde_json::to_string(b).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;}
 for (id,p) in &s.presentation_evidence{db.execute("INSERT OR IGNORE INTO presentation_event_evidence VALUES(?1,?2)",params![id,serde_json::to_string(p).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;}
 db.execute("UPDATE presentation_meta SET revision=revision+1 WHERE id=1",[]).map_err(|e|e.to_string())?;Ok(())
}
pub fn watermark(db:&Connection)->Result<(u64,u64),String>{let revision=db.query_row("SELECT revision FROM presentation_meta WHERE id=1",[],|r|r.get(0)).map_err(|e|e.to_string())?;let cursor=db.query_row("SELECT COALESCE(MAX(rowid),0) FROM events",[],|r|r.get(0)).map_err(|e|e.to_string())?;Ok((revision,cursor))}

fn receipt_valid(s:&Snapshot,j:&Job,e:&Event,p:&EventEvidence)->bool{
 let Some(r)=&p.receipt else{return false};
 if r.source_event_id!=e.event_id||r.verification!="FILES_VERIFIED"||r.files.is_empty()||p.task_id.as_ref()!=Some(&r.to_task_id){return false}
 let Some(edge)=j.workflow.edges.iter().find(|v|v.edge_id==r.edge_id&&e.edge_id.as_ref()==Some(&v.edge_id)&&e.node_id.as_ref()==Some(&v.to_node_id))else{return false};
 if !s.task_executions.iter().any(|b|b.task_id==r.from_task_id&&b.job_id==j.job_id&&b.node_id==edge.from_node_id)||!s.task_executions.iter().any(|b|b.task_id==r.to_task_id&&b.job_id==j.job_id&&b.node_id==edge.to_node_id){return false}
 r.files.iter().all(|f|j.artifacts.iter().any(|a|FileProof::from(a)==*f&&a.node_id==edge.from_node_id&&crate::engine::verify_artifact(a,Path::new(&j.workspace)).is_ok()))
}
fn delivered(j:&Job)->bool{
 j.status=="completed"&&j.nodes.values().all(|n|n.status=="completed")&&!j.artifacts.is_empty()
 &&j.artifacts.iter().all(|a|crate::engine::verify_artifact(a,Path::new(&j.workspace)).is_ok())
 &&j.artifacts.iter().any(|a|a.name=="manifest.json")
 &&(j.practical.is_none()||crate::practical::verify_product(j).is_ok_and(|q|j.quality.as_ref().is_some_and(|saved|saved.machine_status=="PASS"&&saved.artifact_sha256==q.artifact_sha256)))
 &&(!crate::sns::applies(j)||crate::sns::approved(j).is_ok())
}
fn artifact_id(j:&Job,a:&Artifact)->String{format!("artifact:{}",crate::purposes::sha256(&serde_json::to_vec(&(&j.job_id,&a.path)).expect("string tuple")))}
fn task_view(s:&Snapshot,j:&Job,b:&TaskExecutionBinding)->Value{
 let current=j.nodes.get(&b.node_id).filter(|r|r.attempt==b.attempt);
 let(status,reason)=if let Some(r)=current{match r.status.as_str(){
  "queued"=>("IDLE",None),"running"|"handing_off"=>("RUNNING",None),
  "paused"=>("IDLE",Some("PAUSED")),"cancelled"=>("IDLE",Some("CANCELLED")),
  "awaiting_approval"=>if s.approvals.iter().any(|a|a.job_id==j.job_id&&a.node_id==b.node_id&&!a.resolved&&s.presentation_evidence.values().any(|p|p.task_id.as_ref()==Some(&b.task_id)&&p.approval_id.as_ref()==Some(&a.id))){("AWAITING_APPROVAL",None)}else{("UNKNOWN",Some("APPROVAL_EVIDENCE_MISSING"))},
  "completed"=>{let files=j.artifacts.iter().filter(|a|a.node_id==b.node_id).collect::<Vec<_>>();let formal=j.mode!="real"||j.workflow.nodes.iter().find(|n|n.node_id==b.node_id).is_some_and(|n|n.executor!="codex")||b.turn_id.is_some();if formal&&!files.is_empty()&&files.iter().all(|a|crate::engine::verify_artifact(a,Path::new(&j.workspace)).is_ok()){("SUCCEEDED",None)}else{("UNKNOWN",Some("TASK_COMPLETION_UNVERIFIED"))}},
  "failed"=>("FAILED",Some("TASK_FAILED")),_=>("UNKNOWN",Some("FORMAL_RECONCILIATION_REQUIRED"))
 }}else{("UNKNOWN",Some("HISTORICAL_ATTEMPT_STATE_UNRECORDED"))};
 json!({"taskId":b.task_id,"nodeId":b.node_id,"attempt":b.attempt,"roleId":b.role_id,"status":status,"legacyStatus":current.map(|r|&r.status),"reasonCode":reason})
}
fn normalized(s:&Snapshot,j:&Job,e:&Event,p:Option<&EventEvidence>)->(&'static str,Option<&'static str>,bool){
 if e.event_type=="handoff_received"{return if p.is_some_and(|p|receipt_valid(s,j,e,p)){("HANDOFF",None,true)}else{("UNKNOWN",Some("HANDOFF_EVIDENCE_MISSING_OR_INVALID"),false)}}
 if e.event_type=="handoff_started"{return ("RUNNING",Some("HANDOFF_VERIFYING"),false)}
 if e.event_type=="turn_completed"&&["completed","running"].contains(&e.status.as_str()){return ("RUNNING",Some("TURN_ONLY_NOT_DELIVERED"),false)}
 match e.status.as_str(){
  "queued"=>("IDLE",None,false),"running"|"handing_off"=>("RUNNING",None,false),
  "awaiting_approval"=>if p.and_then(|p|p.approval_id.as_ref()).is_some_and(|id|s.approvals.iter().any(|a|&a.id==id&&a.job_id==j.job_id)){("AWAITING_APPROVAL",None,false)}else{("UNKNOWN",Some("APPROVAL_EVIDENCE_MISSING"),false)},
  "completed"=>if e.event_type=="job_completed"&&e.node_id.is_none()&&delivered(j)||e.event_type=="node_completed"&&e.node_id.as_ref().is_some_and(|n|!e.artifact_paths.is_empty()&&e.artifact_paths.iter().all(|path|j.artifacts.iter().any(|a|a.path==*path&&a.node_id==*n&&crate::engine::verify_artifact(a,Path::new(&j.workspace)).is_ok()))){("SUCCEEDED",None,false)}else{("UNKNOWN",Some("SUCCESS_SCOPE_UNVERIFIED"),false)},
  "failed"=>("FAILED",None,false),"paused"=>("IDLE",Some("PAUSED"),false),"cancelled"=>("IDLE",Some("CANCELLED"),false),"disconnected"=>("UNKNOWN",Some("FORMAL_RECONCILIATION_REQUIRED"),false),_=>("UNKNOWN",Some("UNRECOGNIZED_STATUS"),false)
 }
}
pub fn snapshot(db:&Connection,s:&Snapshot,job_id:&str)->Result<Value,String>{
 let j=find_job(s,job_id)?;let(revision,cursor)=watermark(db)?;let purpose=crate::purposes::view(j,s);let mut events=vec![];
 let mut q=db.prepare("SELECT rowid,json FROM events WHERE job_id=?1 ORDER BY rowid").map_err(|e|e.to_string())?;
 let rows=q.query_map([job_id],|r|Ok((r.get::<_,u64>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;
 for row in rows{let(sequence,raw)=row.map_err(|e|e.to_string())?;let e:Event=serde_json::from_str(&raw).map_err(|e|e.to_string())?;let p=s.presentation_evidence.get(&e.event_id);let(status,reason,received)=normalized(s,j,&e,p);
  let binding=p.and_then(|p|p.task_id.as_ref()).and_then(|t|s.task_executions.iter().find(|b|&b.task_id==t&&b.job_id==j.job_id));
  // Artifact handles are stable within the Job; neither local paths nor protocol IDs escape.
  let artifacts=j.artifacts.iter().filter(|a|e.artifact_paths.contains(&a.path)||received&&p.and_then(|p|p.receipt.as_ref()).is_some_and(|r|r.files.iter().any(|f|f.path==a.path))).map(|a|artifact_id(j,a)).collect::<Vec<_>>();
  let handoff=if received{p.and_then(|p|p.receipt.as_ref()).map(|r|json!({"edgeId":r.edge_id,"fromTaskId":r.from_task_id,"toTaskId":r.to_task_id,"sourceEventId":r.source_event_id,"verification":r.verification}))}else{None};
  events.push(json!({"contractVersion":VERSION,"eventId":e.event_id,"sourceEventId":e.event_id,"jobId":j.job_id,"taskId":binding.map(|b|&b.task_id),"purposeId":purpose["purposeId"],"phase":e.node_id.as_deref().unwrap_or("job"),"status":status,"roleId":binding.and_then(|b|b.role_id.as_ref()),"timestamp":e.timestamp,"artifactIds":artifacts,"approvalId":p.and_then(|p|p.approval_id.as_ref()),"sequence":sequence,"scope":if received{"HANDOFF"}else if e.node_id.is_some(){"TASK"}else{"JOB"},"mode":if ["real","demo","local_only","system"].contains(&e.mode.as_str()){e.mode.as_str()}else{"system"},"legacyStatus":e.status,"reasonCode":reason.or(e.error_code.as_deref()),"handoff":handoff}));
 }
 let active=s.job.as_ref().is_some_and(|v|v.job_id==j.job_id);
 let unknown=j.status=="disconnected"||active&&j.mode=="real"&&["running","handing_off"].contains(&j.status.as_str())&&s.connection.status!="connected";
 let job_status=match j.status.as_str(){"queued"|"paused"|"cancelled"=>"IDLE","running"|"handing_off"=>"RUNNING","awaiting_approval"=>if s.approvals.iter().any(|a|a.job_id==j.job_id&&!a.resolved){"AWAITING_APPROVAL"}else{"UNKNOWN"},"completed"=>if delivered(j){"SUCCEEDED"}else{"UNKNOWN"},"failed"=>"FAILED",_=>"UNKNOWN"};
 Ok(json!({"contractVersion":VERSION,"jobId":j.job_id,"revision":revision,"eventCursor":cursor,"events":events,"liveEvidence":if unknown||!active{"UNKNOWN"}else{"CURRENT"},"liveReasonCode":if j.status=="disconnected"{Some("FORMAL_RECONCILIATION_REQUIRED")}else if !active{Some("SAVED_HISTORY_ONLY")}else if unknown{Some("CONNECTION_UNCONFIRMED")}else{None},"purpose":purpose,"roleMapVersion":if crate::sns::applies(j){Some(if crate::sns_team::applies(j){crate::sns_team::ROLE_MAP}else{crate::purposes::ROLE_MAP_VERSION})}else{None},"roles":roles_for(j),"tasks":s.task_executions.iter().filter(|b|b.job_id==j.job_id).map(|b|task_view(s,j,b)).collect::<Vec<_>>(),"jobStatus":job_status,"legacyJobStatus":j.status,"artifacts":j.artifacts.iter().map(|a|json!({"artifactId":artifact_id(j,a),"name":a.name,"sha256":a.sha256,"bytes":a.bytes,"integrity":if crate::engine::verify_artifact(a,Path::new(&j.workspace)).is_ok(){"verified"}else{"failed"}})).collect::<Vec<_>>()}))
}
pub fn notification(db:&Connection,s:&Snapshot,previous:(u64,u64))->Result<Value,String>{let(revision,cursor)=watermark(db)?;Ok(json!({"contractVersion":VERSION,"previousRevision":previous.0,"previousCursor":previous.1,"revision":revision,"eventCursor":cursor,"snapshots":s.job.iter().chain(s.history.iter()).map(|j|snapshot(db,s,&j.job_id)).collect::<Result<Vec<_>,_>>()?}))}

pub fn immutable_event(db:&Connection,e:&Event)->Result<(),String>{let old:Option<String>=db.query_row("SELECT json FROM events WHERE id=?1",[&e.event_id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;if let Some(old)=old{if serde_json::from_str::<Value>(&old).map_err(|e|e.to_string())?!=serde_json::to_value(e).map_err(|e|e.to_string())?{return Err("Persisted Event cannot be replaced".into())}}Ok(())}
