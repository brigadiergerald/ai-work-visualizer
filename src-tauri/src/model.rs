use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub fn now() -> String { chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true) }
pub fn id() -> String { uuid::Uuid::new_v4().to_string() }
pub fn redact(text:&str)->String {
 let bearer=regex::Regex::new(r"(?i)bearer\s+[A-Za-z0-9._~+/=-]+").unwrap();
 let keys=regex::Regex::new(r"(?i)(api[_-]?key|access[_-]?token|refresh[_-]?token|password|secret)\s*[=:]\s*[^\s,;]+").unwrap();
 let tokens=regex::Regex::new(r"\bsk-[A-Za-z0-9_-]{12,}\b").unwrap();
 tokens.replace_all(&keys.replace_all(&bearer.replace_all(text,"Bearer [REDACTED]"),"$1=[REDACTED]"),"[REDACTED]").into_owned()
}
#[derive(Clone,Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase")]
pub struct Node { pub node_id:String,pub role:String,pub label:String,pub sprite_key:String,pub work_instruction:String,pub dependencies:Vec<String>,pub input_artifacts:Vec<String>,pub output_artifacts:Vec<String>,pub executor:String }
#[derive(Clone,Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase")]
pub struct Edge {pub edge_id:String,pub from_node_id:String,pub to_node_id:String}
#[derive(Clone,Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase")]
pub struct Workflow {pub workflow_id:String,pub workflow_name:String,pub nodes:Vec<Node>,pub edges:Vec<Edge>}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Artifact {pub path:String,pub name:String,pub node_id:String,pub sha256:String,pub bytes:u64,pub kind:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Run {pub status:String,pub started_at:Option<String>,pub ended_at:Option<String>,pub output:String,pub error:Option<String>,pub attempt:u32}
impl Default for Run {fn default()->Self{Self{status:"queued".into(),started_at:None,ended_at:None,output:String::new(),error:None,attempt:0}}}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Job {pub job_id:String,pub workflow_id:String,pub workflow:Workflow,pub status:String,pub mode:String,pub instruction:String,pub started_at:String,pub ended_at:Option<String>,pub workspace:String,pub thread_id:Option<String>,pub turn_id:Option<String>,pub active_node_id:Option<String>,pub nodes:HashMap<String,Run>,pub artifacts:Vec<Artifact>,pub stop_requested:bool,#[serde(default)]pub pause_requested:bool,#[serde(default,skip_serializing_if="Option::is_none")]pub practical:Option<crate::practical::Practical>,#[serde(default,skip_serializing_if="Option::is_none")]pub quality:Option<crate::practical::Quality>,#[serde(default,skip_serializing_if="Option::is_none")]pub recovery:Option<crate::recovery::Recovery>,#[serde(default,skip_serializing_if="Option::is_none")]pub collaboration:Option<crate::s2_core::Core>}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Event {pub event_id:String,pub timestamp:String,pub job_id:String,pub workflow_id:String,pub node_id:Option<String>,pub event_type:String,pub status:String,pub message:String,pub artifact_paths:Vec<String>,pub edge_id:Option<String>,pub error_code:Option<String>,pub mode:String}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Approval {pub id:String,pub request_id:serde_json::Value,pub job_id:String,pub node_id:String,pub kind:String,pub title:String,pub details:String,pub params:serde_json::Value,pub resolved:bool,pub decision:Option<String>}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Instruction {pub id:String,pub job_id:String,pub kind:String,pub text:String,pub timestamp:String}
#[derive(Clone,Serialize,Deserialize)]
pub struct Project {pub name:String,pub path:String}
#[derive(Clone,Serialize,Deserialize)]
pub struct Connection {pub status:String,pub version:String,pub account:String,pub error:Option<String>}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Snapshot {#[serde(default,skip_serializing_if="Vec::is_empty")]pub task_executions:Vec<crate::presentation::TaskExecutionBinding>,#[serde(default,skip_serializing_if="HashMap::is_empty")]pub presentation_evidence:HashMap<String,crate::presentation::EventEvidence>,#[serde(default,skip_serializing_if="HashMap::is_empty")]pub purpose_job_context:HashMap<String,crate::purposes::JobContext>,#[serde(default,skip_serializing_if="HashMap::is_empty")]pub completion_plans:HashMap<String,crate::integrations::BoundPlan>,pub workflow:Workflow,pub projects:Vec<Project>,pub project:Project,pub mode:String,pub connection:Connection,pub job:Option<Job>,pub history:Vec<Job>,pub events:Vec<Event>,pub approvals:Vec<Approval>,pub instructions:Vec<Instruction>,pub positions:HashMap<String,serde_json::Value>,pub data_path:String,#[serde(default)]pub settings:crate::setup::Settings}
impl Snapshot {
 pub fn new(data:&std::path::Path)->Self{
  let project=Project{name:"はじまりのギルド".into(),path:data.join("workspace").to_string_lossy().into()};
  Self{task_executions:vec![],presentation_evidence:HashMap::new(),purpose_job_context:HashMap::new(),completion_plans:HashMap::new(),workflow:serde_json::from_str(include_str!("../../src/default-workflow.json")).unwrap(),projects:vec![project.clone()],project:project.clone(),mode:"real".into(),connection:Connection{status:"disconnected".into(),version:String::new(),account:String::new(),error:None},job:None,history:vec![],events:vec![],approvals:vec![],instructions:vec![],positions:HashMap::new(),data_path:data.to_string_lossy().into(),settings:crate::setup::Settings{work_root:project.path.clone(),output_root:data.join("deliveries").to_string_lossy().into(),approval_policy:"on-request".into(),..Default::default()}}
 }
 pub fn event(&mut self,node:Option<&str>,typ:&str,status:&str,message:impl Into<String>,edge:Option<String>,paths:Vec<String>){
  let j=self.job.as_ref();self.events.push(Event{event_id:id(),timestamp:now(),job_id:j.map(|j|j.job_id.clone()).unwrap_or_default(),workflow_id:j.map(|j|j.workflow_id.clone()).unwrap_or(self.workflow.workflow_id.clone()),node_id:node.map(String::from),event_type:typ.into(),status:status.into(),message:redact(&message.into()),artifact_paths:paths,edge_id:edge,error_code:if status=="failed"{Some(typ.into())}else{None},mode:j.map(|j|j.mode.clone()).unwrap_or("system".into())});crate::presentation::capture(self);
 }
}
pub fn transition(from:&str,to:&str)->Result<(),String>{
 let allowed=match from{"queued"=>vec!["running","cancelled","disconnected"],"running"=>vec!["handing_off","awaiting_approval","paused","completed","failed","cancelled","disconnected"],"handing_off"=>vec!["running","completed","failed","cancelled","disconnected"],"awaiting_approval"=>vec!["running","paused","cancelled","failed","disconnected"],"paused"=>vec!["queued","running","cancelled","failed","disconnected"],"failed"|"cancelled"=>vec!["queued","cancelled"],"disconnected"=>vec!["running","paused","failed","cancelled"],"completed"=>vec![],_=>return Err("未知の状態".into())};
 if from==to||allowed.contains(&to){Ok(())}else{Err(format!("不正な状態遷移: {from} → {to}"))}
}
pub fn validate(w:&Workflow)->Result<(),String>{
 if w.workflow_id.trim().is_empty()||w.workflow_name.trim().is_empty()||w.nodes.is_empty()||w.nodes.len()>40{return Err("ワークフロー名と1〜40の工程が必要です。".into());}
 let mut ids=HashSet::new();let mut names=HashSet::new();
 for n in &w.nodes {
  if n.node_id.is_empty()||!n.node_id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'||b==b'-')||!ids.insert(&n.node_id){return Err("工程IDが不正または重複しています。".into());}
  if n.label.trim().is_empty()||n.role.trim().is_empty()||n.work_instruction.trim().is_empty()||!["codex","manual","save","quality","review"].contains(&n.executor.as_str())||!["scholar","scribe","artist","analyst","messenger"].contains(&n.sprite_key.as_str()){return Err("工程名・役割・指示・実行方法・キャラクターを確認してください。".into());}
  if n.executor=="codex"&&(n.output_artifacts.len()!=1||!n.output_artifacts.iter().all(|x|[".md",".txt",".json",".html"].iter().any(|ext|x.ends_with(ext)))){return Err("Codex工程はMarkdown・テキスト・JSON・HTMLの1ファイルで指定してください。".into());}
  for f in &n.output_artifacts {if f.is_empty()||f.contains(['/', '\\', ':'])||f.contains("..")||!names.insert(f){return Err("出力名は重複しない通常のファイル名にしてください。".into());}}
 }
 let mut edges=HashSet::new();let mut pairs=HashSet::new();
 for e in &w.edges{if e.edge_id.is_empty()||!edges.insert(&e.edge_id)||!pairs.insert((&e.from_node_id,&e.to_node_id))||!ids.contains(&e.from_node_id)||!ids.contains(&e.to_node_id){return Err("接続IDの重複、重複接続、存在しない工程があります。".into());}}
 for n in &w.nodes{let deps:HashSet<_>=n.dependencies.iter().collect();let incoming:HashSet<_>=w.edges.iter().filter(|e|e.to_node_id==n.node_id).map(|e|&e.from_node_id).collect();if deps.len()!=n.dependencies.len()||deps!=incoming{return Err(format!("{}の依存関係と接続線が一致しません。",n.label));}}
 let mut done=HashSet::new();while done.len()<w.nodes.len(){let before=done.len();for n in &w.nodes{if n.dependencies.iter().all(|d|done.contains(d)){done.insert(n.node_id.clone());}}if done.len()==before{return Err("依存関係が循環しています。".into());}}
 Ok(())
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn rejects_terminal_transition(){assert!(transition("completed","running").is_err());assert!(transition("running","disconnected").is_ok());}
 #[test] fn validates_graph_and_rejects_cycle(){let mut w:Workflow=serde_json::from_str(include_str!("../../src/default-workflow.json")).unwrap();assert!(validate(&w).is_ok());w.nodes[0].dependencies.push("delivery".into());w.edges.push(Edge{edge_id:"cycle".into(),from_node_id:"delivery".into(),to_node_id:"research".into()});assert!(validate(&w).unwrap_err().contains("循環"));}
}
