//! Versioned dashboard boundary. Protocol interpretation stays on the host.
//! Agent identity is descriptive; only the Codex adapter exists in this version.
use crate::{engine::Engine,model::{Job,Snapshot},sns};
use serde::Deserialize;
use serde_json::{json,Value};
use std::{collections::HashMap,sync::{Arc,atomic::Ordering},path::Path,fs};
fn state(s:&str)->&str{match s{"disconnected"=>"unknown","handing_off"=>"running",s if ["queued","running","awaiting_approval","paused","completed","failed","cancelled"].contains(&s)=>s,_=>"unknown"}}
fn job(j:&Job,s:&Snapshot)->Value{
 let approvals=s.approvals.iter().filter(|a|a.job_id==j.job_id&&!a.resolved).map(|a|{
  let kind=if a.kind=="content_review"{"content"}else if a.kind.ends_with("requestUserInput"){"questions"}else if a.kind=="manual_artifact"{"file"}else{"operation"};
  let questions=a.params["questions"].as_array().map(|qs|qs.iter().map(|q|json!({"id":q["id"],"prompt":q["question"],"secret":q["isSecret"]==true,"options":q["options"].as_array().map(|os|os.iter().map(|o|o["label"].clone()).collect::<Vec<_>>()).unwrap_or_default()})).collect::<Vec<_>>()).unwrap_or_default();
  json!({"id":a.id,"kind":kind,"title":a.title,"details":a.details,"questions":questions,"responsePending":a.decision.is_some()})
 }).collect::<Vec<_>>();
 let events=s.events.iter().filter(|e|e.job_id==j.job_id).map(|e|json!({"id":e.event_id,"at":e.timestamp,"stageId":e.node_id,"state":state(&e.status),"message":e.message,"code":e.event_type})).collect::<Vec<_>>();
 let product_ok=crate::practical::verify_product(j).is_ok();
 json!({"id":j.job_id,"instruction":j.instruction,"agent":{"id":"codex","label":"Codex"},"workflowId":if sns::applies(j){sns::ID}else{&j.workflow_id},"workflowName":j.workflow.workflow_name,"state":state(&j.status),"mode":j.mode,"startedAt":j.started_at,"endedAt":j.ended_at,"outputDirectory":j.workspace,"activeStageId":j.active_node_id,
 "stages":j.workflow.nodes.iter().map(|n|json!({"id":n.node_id,"label":n.label,"executor":if n.executor=="codex"{"agent"}else if n.executor=="review"{"human"}else{"host"},"state":state(j.nodes.get(&n.node_id).map(|r|r.status.as_str()).unwrap_or("unknown")),"error":j.nodes.get(&n.node_id).and_then(|r|r.error.clone())})).collect::<Vec<_>>(),
 "approvals":approvals,"logs":events,"artifacts":j.artifacts.iter().map(|a|json!({"name":a.name,"path":a.path,"sha256":a.sha256,"bytes":a.bytes,"role":a.kind,"integrity":if crate::engine::verify_artifact(a,Path::new(&j.workspace)).is_ok(){"verified"}else{"failed"}})).collect::<Vec<_>>(),
 "quality":{"machine":j.quality.as_ref().map(|q|q.machine_status.as_str()).unwrap_or("NOT_VERIFIED"),"display":j.quality.as_ref().map(|q|q.display_status.as_str()).unwrap_or("NOT_VERIFIED"),"filesVerified":product_ok},
 "recovery":{"required":j.status=="disconnected","localReview":sns::applies(j)&&j.active_node_id.as_deref()==Some(sns::REVIEW),"recorded":j.recovery.is_some(),"message":if j.status=="disconnected"{"UNKNOWN：元の実行・保存結果の照合が必要です。自動再送しません。"}else{""}},
 "decisions":crate::decisions::view(j,s),"publication":"NOT_PERFORMED","completionPlan":s.completion_plans.get(&j.job_id),"purpose":crate::purposes::view(j,s)})
}
pub fn snapshot(s:&Snapshot)->Value{json!({"contractVersion":1,"connection":{"agentId":"codex","state":s.connection.status,"version":s.connection.version,"account":s.connection.account,"error":s.connection.error},"currentJobId":s.job.as_ref().map(|j|&j.job_id),"jobs":s.job.iter().chain(s.history.iter().rev()).map(|j|job(j,s)).collect::<Vec<_>>(),"settings":s.settings,"mode":s.mode,"dataPath":s.data_path})}
#[derive(Deserialize)]
#[serde(tag="action",rename_all="camelCase",rename_all_fields="camelCase",deny_unknown_fields)]
pub enum Action{
 Posting{request:crate::posting::Action},
 ListPurposes,
 StartPurpose{purpose_id:String,definition_version:String,input:Value,#[serde(default)]completion_plan:Option<crate::integrations::PlanInput>},
 Connectors{request:crate::connectors::Action},
 Providers{request:crate::providers::Action},
 Browser{request:crate::browser::Action},
 Integrations{request:crate::integrations::Action},
 Connect,
 Configure{settings:crate::setup::Settings},
 Inspect{path:String},
 Start{workflow_id:String,instruction:String,#[serde(default)]completion_plan:Option<crate::integrations::PlanInput>},
 Control{job_id:String,operation:String},
 Answer{job_id:String,approval_id:String,decision:String,#[serde(default)]text:String,answers:Option<HashMap<String,Vec<String>>>},
 ReadText{job_id:String,path:String,purpose:String},
 OpenArtifact{job_id:String,path:String},
}
fn find<'a>(s:&'a Snapshot,id:&str)->Result<&'a Job,String>{s.job.iter().chain(s.history.iter()).find(|j|j.job_id==id).ok_or("Jobがありません".into())}
pub(crate) fn start_prepared_purpose(e:&Arc<Engine>,prepared:crate::purposes::Prepared,completion_plan:Option<crate::integrations::PlanInput>)->Result<Value,String>{
 let before=e.snapshot();
 match e.start_purpose(prepared,completion_plan.unwrap_or_default()){
  Ok(id)=>Ok(json!({"jobId":id})),
  Err(error)=>{let after=e.snapshot();if before.job.as_ref().map(|j|&j.job_id)==after.job.as_ref().map(|j|&j.job_id)&&before.purpose_job_context==after.purpose_job_context{Err(format!("[PURPOSE_NOT_STARTED] {error}"))}else{Err(error)}}
 }
}
pub fn perform(e:&Arc<Engine>,a:Action)->Result<Value,String>{
 match a{
  Action::Posting{request}=>crate::posting::perform(e,request),
  Action::ListPurposes=>Ok(crate::purposes::catalog()),
  Action::StartPurpose{purpose_id,definition_version,input,completion_plan}=>{
   if definition_version!=crate::purposes::VERSION{return Err("[PURPOSE_NOT_STARTED] Purpose定義versionが一致しません。用途一覧を再取得してください".into())}
   if e.snapshot().mode!="real"{return Err("[PURPOSE_NOT_STARTED] 用途からの開始は実行モードのみです".into())}
   let prepared=crate::purposes::prepare(&purpose_id,&definition_version,input).map_err(|e|format!("[PURPOSE_NOT_STARTED] {e}"))?;
   start_prepared_purpose(e,prepared,completion_plan)
  },
  Action::Connectors{request}=>crate::connectors::perform(e,request),
  Action::Providers{request}=>crate::providers::perform(e,request),
  Action::Browser{request}=>crate::browser::perform(e,request),
  Action::Integrations{request}=>crate::integrations::perform(e,request),
  Action::Connect=>{e.connect()?;Ok(Value::Null)},
  Action::Inspect{path}=>Ok(crate::setup::inspect(&path)),
  Action::Configure{settings}=>{e.save_setup(settings)?;Ok(Value::Null)},
  Action::Start{workflow_id,instruction,completion_plan}=>{
   if e.snapshot().mode!="real"{return Err("無料画面は実行モードのみです。デモ記録は履歴として参照できます".into())}
   let w=match workflow_id.as_str(){sns::ID=>Some(sns::autonomous_workflow()),crate::general::ID=>None,_=>return Err("未対応Workflowです。別エージェントは未実装です".into())};
   Ok(json!({"jobId":e.start_workflow_with_completion(instruction,w,Some(completion_plan.unwrap_or_default()))?}))
  },
  Action::Control{job_id,operation}=>{
   let s=e.snapshot();if s.job.as_ref().is_none_or(|j|j.job_id!=job_id){return Err("現在のJobだけ操作できます".into())}
   if operation=="recoverReview"{if e.worker.load(Ordering::SeqCst){return Err("実行中です".into())}sns::recover(e,&job_id)?;}else if ["stop","pause","resume","retry","reconcile","interrupt_unknown"].contains(&operation.as_str()){e.control(operation)?;}else{return Err("未対応の操作です".into())}Ok(Value::Null)
  },
  Action::Answer{job_id,approval_id,decision,text,answers}=>{
   let s=e.snapshot();let a=s.approvals.iter().find(|a|a.id==approval_id&&a.job_id==job_id&&!a.resolved).ok_or("Jobと承認要求が一致しません")?;
   if a.kind=="content_review"{e.answer_content_review(&job_id,&approval_id,&decision)?;}else{e.answer_structured(approval_id,decision,text,answers)?;}Ok(Value::Null)
  },
  Action::ReadText{job_id,path,purpose}=>{
   let s=e.snapshot();let j=find(&s,&job_id)?;let a=j.artifacts.iter().find(|a|a.path==path&&a.kind=="product").ok_or("登録済み完成品ではありません")?;
   if !["preview","copy"].contains(&purpose.as_str())||!a.name.ends_with(".txt")&& !a.name.ends_with(".md"){return Err("テキスト完成品のみを読み取れます".into())}
   crate::practical::verify_product(j)?;
   if purpose=="copy"{if j.status!="completed"{return Err("納品完了前はコピーできません".into())}sns::approved(j)?;}
   crate::engine::verify_artifact(a,Path::new(&j.workspace))?;
   Ok(json!({"text":fs::read_to_string(&a.path).map_err(|e|e.to_string())?,"sha256":a.sha256}))
  },
  Action::OpenArtifact{job_id,path}=>{let s=e.snapshot();let j=find(&s,&job_id)?;if !j.artifacts.iter().any(|a|a.path==path){return Err("別Jobの成果物です".into())}if sns::applies(j){sns::approved(j)?;}e.open_artifact(path)?;Ok(Value::Null)}
 }
}
