use crate::{model::*,rpc::Rpc,storage::Store};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,path::{Path,PathBuf},sync::{Arc,Mutex,mpsc,atomic::{AtomicBool,Ordering}},time::Duration};
use tauri::{AppHandle,Emitter};
pub struct Data {pub snapshot:Snapshot,pub store:Store}
pub struct Engine {pub(crate) presentation_delivery_ok:AtomicBool,pub(crate) profile_root:PathBuf,pub data:Mutex<Data>,pub rpc:Mutex<Option<Arc<Rpc>>>,pub worker:AtomicBool,pub connecting:AtomicBool,pub app:Option<AppHandle>,pub(crate) response_gate:Mutex<()>,secrets:Mutex<Vec<String>>,_s2_lease:Option<fs::File>,_profile_lease:fs::File}
impl Engine{
 pub fn open(path:&Path,app:Option<AppHandle>)->Result<Arc<Self>,String>{
  fs::create_dir_all(path).map_err(|e|e.to_string())?;
  let profile_lease=fs::OpenOptions::new().create(true).read(true).write(true).open(path.join(".profile-lease")).map_err(|e|e.to_string())?;profile_lease.try_lock().map_err(|_|"同じ利用者プロファイルは既に起動しています。元のアプリを確認してください。二重実行は開始しません。")?;
  let mut store=Store::open(&path.join("guild.sqlite3"))?;
  let mut s=store.load()?.unwrap_or_else(||Snapshot::new(path));fs::create_dir_all(&s.project.path).map_err(|e|e.to_string())?;
  if !s.settings.setup_complete{if s.settings.work_root.is_empty(){s.settings.work_root=s.project.path.clone();}if s.settings.output_root.is_empty(){s.settings.output_root=path.join("deliveries").to_string_lossy().into();}if s.settings.approval_policy.is_empty(){s.settings.approval_policy="on-request".into();}}
  if !s.settings.output_root.is_empty()&&Path::new(&s.settings.output_root).starts_with(path){fs::create_dir_all(&s.settings.output_root).map_err(|e|e.to_string())?;}
  let s2_lease=if s.job.as_ref().is_some_and(|j|j.collaboration.is_some()){let f=fs::OpenOptions::new().create(true).read(true).write(true).open(path.join(".s2-core-lease")).map_err(|e|e.to_string())?;f.try_lock().map_err(|_|"同じ画像付きJobは別のEngineで使用中です。元のアプリ終了後に復旧してください。")?;Some(f)}else{None};
  s.connection.status="disconnected".into();s.connection.account=String::new();
  if s.job.as_ref().is_some_and(|j|["running","handing_off","awaiting_approval"].contains(&j.status.as_str())){
   let j=s.job.as_mut().unwrap();j.status="disconnected".into();for n in j.nodes.values_mut(){if ["running","handing_off","awaiting_approval"].contains(&n.status.as_str()){n.status="disconnected".into();}}
   let message=if j.collaboration.is_some(){"アプリ再起動：同じ画像Taskの受領intent・保存file・hashを照合してください。推論やdownloadは再実行しません。"}else{"アプリ再起動：前回の実行結果は未確認です。元スレッドの照合が必要です。自動再送はしません。"};
   s.event(None,"restart_recovery","disconnected",message,None,vec![]);
  }
  store.save(&s)?;Ok(Arc::new(Self{presentation_delivery_ok:AtomicBool::new(true),profile_root:path.to_path_buf(),data:Mutex::new(Data{snapshot:s,store}),rpc:Mutex::new(None),worker:AtomicBool::new(false),connecting:AtomicBool::new(false),app,response_gate:Mutex::new(()),secrets:Mutex::new(vec![]),_s2_lease:s2_lease,_profile_lease:profile_lease}))
 }
 pub fn presentation_snapshot(&self,job_id:&str)->Result<Value,String>{let d=self.data.lock().map_err(|_|"状態ロックエラー")?;let mut frame=crate::presentation::snapshot(&d.store.0,&d.snapshot,job_id)?;if !self.presentation_delivery_ok.load(Ordering::SeqCst){frame["liveEvidence"]=json!("RESYNC_REQUIRED");frame["liveReasonCode"]=json!("NOTIFICATION_DELIVERY_FAILED");}Ok(frame)}
 pub fn snapshot(&self)->Snapshot{self.data.lock().unwrap().snapshot.clone()}
 pub fn update<T>(&self,f:impl FnOnce(&mut Snapshot)->Result<T,String>)->Result<T,String>{
  let mut d=self.data.lock().map_err(|_|"状態ロックエラー")?;let mut next=d.snapshot.clone();let result=f(&mut next)?;
  crate::presentation::capture_new_executions(&d.snapshot,&mut next)?;
  if let(Some(a),Some(b))=(&d.snapshot.job,&next.job){if a.job_id==b.job_id{transition(&a.status,&b.status)?;for(id,run)in &b.nodes{if let Some(old)=a.nodes.get(id){transition(&old.status,&run.status)?;}}}}
  // Some existing local review/recovery paths commit completion without an Event.
  // Capture only a NEW bound execution transition, never backfill legacy history.
  let completed=next.job.as_ref().map(|j|j.nodes.iter().filter(|(id,r)|r.status=="completed"&&d.snapshot.job.as_ref().filter(|old|old.job_id==j.job_id).and_then(|old|old.nodes.get(*id)).is_some_and(|old|old.status!="completed")&&next.task_executions.iter().any(|b|b.job_id==j.job_id&&b.node_id==**id&&b.attempt==r.attempt)&&!next.events.iter().skip(d.snapshot.events.len()).any(|e|e.event_type=="node_completed"&&e.node_id.as_ref()==Some(*id))).map(|(id,_)|id.clone()).collect::<Vec<_>>()).unwrap_or_default();
  for node in completed{let paths=next.job.as_ref().unwrap().artifacts.iter().filter(|a|a.node_id==node).map(|a|a.path.clone()).collect();next.event(Some(&node),"node_completed","completed","保存された工程完了を記録しました。Job全体の完了とは区別します。",None,paths);}
  scrub_snapshot(&mut next,&self.secrets.lock().unwrap());
  let previous=crate::presentation::watermark(&d.store.0)?;
  d.store.save(&next)?;d.snapshot=next;if let Some(app)=&self.app{
   // Notification failures do not turn a committed Job into FAILED. A consumer
   // must resync on observed loss/gaps; silence alone proves nothing.
   let core_ok=app.emit("core:snapshot",crate::core_api::snapshot(&d.snapshot)).is_ok();
   let guild_ok=app.emit("guild:snapshot",&d.snapshot).is_ok();
   let ok=crate::presentation::notification(&d.store.0,&d.snapshot,previous).and_then(|frame|app.emit("core:presentation",frame).map_err(|e|e.to_string())).is_ok();
   self.presentation_delivery_ok.store(ok&&core_ok&&guild_ok,Ordering::SeqCst);
  }Ok(result)
 }
 pub fn connect(self:&Arc<Self>)->Result<(),String>{
  if self.connecting.swap(true,Ordering::SeqCst){return Err("接続確認中です。".into())}
  if self.worker.load(Ordering::SeqCst){self.connecting.store(false,Ordering::SeqCst);return Err("実行中の接続は変更できません。".into())}
  self.update(|s|{s.connection.status="connecting".into();s.connection.error=None;Ok(())})?;
  let result=Rpc::spawn_for(&self.snapshot().settings.codex_path);self.connecting.store(false,Ordering::SeqCst);
  match result{Ok((r,version))=>{let old=self.rpc.lock().unwrap().replace(r.clone());if let Some(o)=old{o.shutdown();}let updated=self.update(|s|{s.connection=Connection{status:"connected".into(),version,account:"ChatGPTアカウント".into(),error:None};s.event(None,"connection","queued","Codex app-serverとstdio接続。ChatGPT認証を確認しました。",None,vec![]);Ok(())});self.watch_connection(&r);updated},Err(e)=>{self.update(|s|{s.connection.status="disconnected".into();s.connection.error=Some(e.clone());s.event(None,"connection_error","failed",e.clone(),None,vec![]);Ok(())})?;Err(e)}}
 }
 fn watch_connection(self:&Arc<Self>,rpc:&Arc<Rpc>){
  let owner=Arc::downgrade(self);let observed=Arc::downgrade(rpc);
  std::thread::spawn(move||loop{std::thread::sleep(Duration::from_secs(1));let(Some(e),Some(r))=(owner.upgrade(),observed.upgrade())else{return};
   if !e.rpc.lock().unwrap().as_ref().is_some_and(|current|Arc::ptr_eq(current,&r)){return}
   if !r.alive.load(Ordering::SeqCst){let _=e.update(|s|{s.connection.status="disconnected".into();s.connection.error=Some("app-serverの接続が終了しました。".into());if let Some(j)=s.job.as_mut(){if j.mode=="real"&&["running","handing_off","awaiting_approval"].contains(&j.status.as_str()){j.status="disconnected".into();if let Some(n)=j.active_node_id.as_ref().and_then(|id|j.nodes.get_mut(id)){if n.status!="completed"{n.status="disconnected".into();}}}}s.event(None,"connection_closed","disconnected","app-serverの接続終了を確認しました。未確認の実行は完了にしません。",None,vec![]);Ok(())});return}
  });
 }
 pub fn get_rpc(&self)->Result<Arc<Rpc>,String>{self.rpc.lock().unwrap().as_ref().filter(|r|r.alive.load(Ordering::SeqCst)).cloned().ok_or("Codexに接続してください。".into())}
 pub fn start(self:&Arc<Self>,instruction:String)->Result<String,String>{self.start_workflow(instruction,None)}
 pub fn start_workflow(self:&Arc<Self>,instruction:String,selected:Option<Workflow>)->Result<String,String>{self.start_workflow_with_completion(instruction,selected,None)}
 pub fn start_workflow_with_completion(self:&Arc<Self>,instruction:String,selected:Option<Workflow>,plan:Option<crate::integrations::PlanInput>)->Result<String,String>{
  self.start_with_context(instruction,selected,plan,None)
 }
 pub fn start_purpose(self:&Arc<Self>,prepared:crate::purposes::Prepared,plan:crate::integrations::PlanInput)->Result<String,String>{
  self.start_with_context(prepared.instruction,Some(prepared.workflow),Some(plan),Some(prepared.context))
 }
 fn start_with_context(self:&Arc<Self>,instruction:String,selected:Option<Workflow>,plan:Option<crate::integrations::PlanInput>,context:Option<crate::purposes::JobContext>)->Result<String,String>{
  let completion=plan.map(|p|crate::integrations::bind_plan(self,p)).transpose()?;
  if instruction.trim().is_empty()||instruction.len()>64000{return Err("指示を1〜64,000バイトで入力してください。".into())}
  if self.app.is_some()&&self.snapshot().mode=="real"&&!self.snapshot().settings.setup_complete{return Err("設定から作業範囲と納品先を保存し、Codex接続を確認してください。".into())}
  if self.worker.swap(true,Ordering::SeqCst){return Err("既に実行中です。".into())}
  let result=(||{let mut s=self.snapshot();let mut practical=if selected.is_some(){crate::general::intake_generic(&instruction)?}else if s.mode=="real"{if s.settings.setup_complete{crate::general::intake(&instruction)?}else{crate::practical::intake(&instruction)?}}else{None};if let Some(p)=&practical{s.workflow=crate::practical::workflow_for(p);}if let Some(w)=&selected{s.workflow=w.clone();}validate(&s.workflow)?;if [crate::sns_team::ID,crate::sns_team::DECISIONS_ID].contains(&s.workflow.workflow_id.as_str()){if let Some(p)=&mut practical{p.max_turn_starts=3;}}
   if s.job.as_ref().is_some_and(|j|["running","awaiting_approval","paused","disconnected","handing_off"].contains(&j.status.as_str())){return Err("現在のクエストを停止、復旧、または再開してから新規作成してください。".into())}
   if s.mode=="real"{self.get_rpc()?;}
   let inputs=if s.settings.setup_complete&&s.mode=="real"{crate::general::inputs(&s.settings)?}else{vec![]};
   let job_id=id();let root=if s.settings.setup_complete{&s.settings.output_root}else{&s.project.path};let root=safe_project(root.clone())?;let workspace=job_workspace(&root,&job_id)?;
   self.update(|s|{if let Some(j)=s.job.take(){s.history.push(j);}let workflow=if let Some(w)=&selected{w.clone()}else if let Some(p)=&practical{crate::practical::workflow_for(p)}else{s.workflow.clone()};let nodes=workflow.nodes.iter().map(|n|(n.node_id.clone(),Run::default())).collect();
    s.job=Some(Job{job_id:job_id.clone(),workflow_id:workflow.workflow_id.clone(),workflow,status:"running".into(),mode:s.mode.clone(),instruction:instruction.clone(),started_at:now(),ended_at:None,workspace:workspace.to_string_lossy().into(),thread_id:None,turn_id:None,active_node_id:None,nodes,artifacts:vec![],stop_requested:false,pause_requested:false,practical:practical.clone(),quality:None,recovery:None,collaboration:None});
    if let Some(plan)=&completion{s.completion_plans.insert(job_id.clone(),plan.clone());}
    if let Some(c)=&context{let mut c=c.clone();c.job_id=job_id.clone();crate::purposes::validate_context(&c,s.job.as_ref().unwrap())?;if s.purpose_job_context.insert(job_id.clone(),c).is_some(){return Err("Purpose contextが重複しています".into())}}
    s.instructions.push(Instruction{id:id(),job_id:job_id.clone(),kind:"initial".into(),text:instruction,timestamp:now()});s.event(None,"job_started","running",if s.mode=="demo"{"デモのクエストを開始しました。AI推論は実行しません。"}else{"クエストを開始しました。1つのCodex環境で工程を順番に実行します。"},None,vec![]);Ok(())})?;
   if !inputs.is_empty(){let n=self.snapshot().job.as_ref().unwrap().workflow.nodes.iter().find(|n|n.node_id==if [crate::sns_team::ID,crate::sns_team::DECISIONS_ID].contains(&s.workflow.workflow_id.as_str()){"research"}else{"plan"}).cloned().ok_or("入力を受け取る工程がありません")?;for(name,bytes)in inputs{if let Err(error)=self.write_artifact(&n,&name,&bytes,"input"){self.update(|s|{s.job.as_mut().unwrap().status="failed".into();s.event(Some(&n.node_id),"input_copy_failed","failed",error.clone(),None,vec![]);Ok(())})?;return Err(error)}}}Ok(job_id)})();
  match result{Ok(id)=>{self.spawn_worker();Ok(id)},Err(e)=>{self.worker.store(false,Ordering::SeqCst);Err(e)}}
 }
 fn spawn_worker(self:&Arc<Self>){let engine=self.clone();std::thread::spawn(move||{let result=engine.run();if let Err(e)=result{let _=engine.update(|s|{if let Some(j)=&mut s.job{if !["completed","cancelled","paused","disconnected"].contains(&j.status.as_str()){j.status="failed".into();j.ended_at=Some(now());if let Some(id)=&j.active_node_id{let n=j.nodes.get_mut(id).unwrap();if n.status!="completed"{n.status="failed".into();n.error=Some(redact(&e));n.ended_at=Some(now());}}}}s.event(None,"execution_error","failed",e,None,vec![]);Ok(())});}engine.worker.store(false,Ordering::SeqCst);});}
 fn run(self:&Arc<Self>)->Result<(),String>{
  if self.snapshot().job.as_ref().is_some_and(|j|j.collaboration.is_some()){return Err("画像付きJobは元Taskの受領・統合・復旧経路だけで実行します。Codex再送はしません。".into())}
  self.update(|s|{if let Some(j)=s.job.as_mut(){if j.status=="queued"{j.status="running".into();}}Ok(())})?;
  loop{let s=self.snapshot();let j=s.job.as_ref().ok_or("ジョブがありません")?;
   if j.stop_requested||j.status=="cancelled"{self.cancelled()?;return Ok(())}
   if j.pause_requested{self.paused()?;return Ok(())}
   if ["paused","disconnected"].contains(&j.status.as_str()){return Ok(())}
   let next=j.workflow.nodes.iter().find(|n|j.nodes[&n.node_id].status=="queued"&&n.dependencies.iter().all(|d|j.nodes[d].status=="completed")).cloned();
   let Some(n)=next else{if j.nodes.values().all(|n|n.status=="completed"){if j.practical.is_some(){let q=crate::practical::verify_product(j)?;if j.quality.as_ref().is_none_or(|old|old.artifact_sha256!=q.artifact_sha256||old.machine_status!="PASS"){return Err("完成品QAが欠落・hash不一致です".into())}}self.update(|s|{let j=s.job.as_mut().unwrap();j.status="completed".into();j.active_node_id=None;j.ended_at=Some(now());s.event(None,"job_completed","completed","工程と登録ファイルの検査が終了しました。完成品の表示確認と実運用E2Eは別判定です。",None,vec![]);Ok(())})?;if let Err(err)=crate::integrations::production_completed(self,&j.job_id){self.update(|s|{s.event(None,"optional_delivery_error","failed",err,None,vec![]);Ok(())})?;}return Ok(())}return Err("実行可能な工程がありません。状態を確認してください。".into())};
   if j.mode!="demo"{if let Err(err)=crate::practical::required_inputs(j,&n){self.update(|s|{s.event(Some(&n.node_id),"input_gate_failed","failed",err.clone(),None,vec![]);Ok(())})?;return Err(err)}}
   self.update(|s|{let j=s.job.as_mut().unwrap();j.active_node_id=Some(n.node_id.clone());let r=j.nodes.get_mut(&n.node_id).unwrap();r.status="running".into();r.started_at=Some(now());r.ended_at=None;r.attempt+=1;j.status="running".into();crate::presentation::begin_task(s,&n.node_id)?;s.event(Some(&n.node_id),"node_started","running",format!("{}：{}を開始。",n.role,n.label),None,vec![]);Ok(())})?;
   for e in j.workflow.edges.iter().filter(|e|e.to_node_id==n.node_id){self.update(|s|{s.event(Some(&n.node_id),"handoff_started","handing_off","入力成果物を受け渡しています。",Some(e.edge_id.clone()),vec![]);Ok(())})?;
    // The acknowledgement follows actual validation of every dependency's registered files.
    for a in j.artifacts.iter().filter(|a|a.node_id==e.from_node_id){if let Err(err)=verify_artifact(a,Path::new(&j.workspace)){self.update(|s|{s.event(Some(&n.node_id),"handoff_failed","failed",err.clone(),Some(e.edge_id.clone()),vec![a.path.clone()]);Ok(())})?;return Err(err)}}
    let files=j.artifacts.iter().filter(|a|a.node_id==e.from_node_id).cloned().collect::<Vec<_>>();
    self.update(|s|{s.event(Some(&n.node_id),"handoff_received","queued","入力成果物の実ファイルとSHA-256を確認しました。",Some(e.edge_id.clone()),vec![]);crate::presentation::receive(s,&e.edge_id,&files)?;Ok(())})?;
   }
   if n.executor=="review"{crate::sns::request(self,&n)?;return Ok(())}else if j.mode=="demo"{self.demo_node(&n)?;}else if n.executor=="manual"{self.manual_node(&n)?;}else if n.executor=="save"{if crate::sns_team::applies(j){crate::sns_team::check_connectors(self,&n)?;}self.save_manifest(&n)?;}else if n.executor=="quality"{self.quality_node(&n)?;}else{self.codex_node(&n)?;}
   let current=self.snapshot();let j=current.job.as_ref().unwrap();if j.stop_requested{self.cancelled()?;return Ok(())}if j.pause_requested{self.paused()?;return Ok(())}if ["paused","disconnected","cancelled"].contains(&j.status.as_str()){return Ok(())}
   if !j.artifacts.iter().any(|a|a.node_id==n.node_id){return Err("この工程の実ファイル成果物を確認できないため、完了にできません。".into())}
   // Codex's formal turn result and saved artifact both passed before this point.
   self.update(|s|{let j=s.job.as_mut().unwrap();let r=j.nodes.get_mut(&n.node_id).unwrap();r.status="completed".into();r.ended_at=Some(now());j.turn_id=None;let paths=j.artifacts.iter().filter(|a|a.node_id==n.node_id).map(|a|a.path.clone()).collect();s.event(Some(&n.node_id),"node_completed","completed",format!("{}が完了しました。",n.label),None,paths);Ok(())})?;
  }
 }
 pub(crate) fn write_artifact(&self,n:&Node,name:&str,bytes:&[u8],kind:&str)->Result<Artifact,String>{
  let cleaned;if kind=="text"{cleaned=self.scrub(&String::from_utf8_lossy(bytes)).into_bytes();}else{cleaned=bytes.to_vec();}let bytes=cleaned.as_slice();
  let s=self.snapshot();let j=s.job.as_ref().ok_or("ジョブがありません")?;if name.is_empty()||name.contains(['/', '\\', ':'])||name.contains(".."){return Err("不正な出力名".into())}
  let attempt=j.nodes.get(&n.node_id).map(|r|r.attempt).unwrap_or(1);
  let folder=if attempt>1{Path::new(&j.workspace).join(format!("{}-attempt-{attempt}",n.node_id))}else{PathBuf::from(&j.workspace)};
  fs::create_dir_all(&folder).map_err(|e|e.to_string())?;
  let p=folder.join(name);if p.exists()&&j.recovery.is_none(){return Err(format!("既存成果物を上書きしません: {name}"))}
  crate::recovery::install_exact(&p,bytes)?;
  let a=artifact(&p,&n.node_id,kind)?;self.update(|s|{let j=s.job.as_mut().unwrap();if let Some(old)=j.artifacts.iter().find(|old|old.path==a.path){if old.sha256!=a.sha256||old.node_id!=a.node_id||old.bytes!=a.bytes{return Err("登録済み成果物と再開fileが異なります".into())}}else{j.artifacts.push(a.clone())}Ok(())})?;Ok(a)
 }
 fn demo_node(&self,n:&Node)->Result<(),String>{
  for _ in 0..12{std::thread::sleep(Duration::from_millis(150));if self.snapshot().job.as_ref().is_some_and(|j|j.stop_requested){return Ok(())}if self.snapshot().job.as_ref().is_some_and(|j|j.pause_requested){self.paused()?;return Ok(())}}
  let text=format!("# デモ成果物：{}\n\nこれはUI確認用のサンプルです。Codexの推論結果ではありません。\n\n指示：{}\n",n.label,self.snapshot().job.as_ref().unwrap().instruction);
  self.write_artifact(n,&format!("demo-{}.md",n.node_id),text.as_bytes(),"demo")?;self.update(|s|{s.job.as_mut().unwrap().nodes.get_mut(&n.node_id).unwrap().output=text;s.event(Some(&n.node_id),"demo_step","running","デモの状態遷移を記録。実際のAI実行ではありません。",None,vec![]);Ok(())})
 }
 pub(crate) fn save_manifest(&self,n:&Node)->Result<(),String>{
  let s=self.snapshot();let j=s.job.as_ref().unwrap();for a in &j.artifacts{verify_artifact(a,Path::new(&j.workspace))?;}
  if j.practical.is_some(){let q=crate::practical::verify_product(j)?;if j.quality.as_ref().is_none_or(|old|old.artifact_sha256!=q.artifact_sha256||old.machine_status!="PASS"){return Err("完成品QA未合格のため保存できません".into())}}
  crate::sns::approved(j)?;
  let artifacts=j.artifacts.iter().filter(|a|if crate::sns_team::applies(j){a.name!="manifest.json"}else{a.node_id!=n.node_id}).collect::<Vec<_>>();
  let mut value=json!({"jobId":j.job_id,"mode":j.mode,"createdAt":now(),"publication":"NOT_PERFORMED","artifacts":artifacts});
  let existing=Path::new(&j.workspace).join("manifest.json");
  if j.recovery.is_some()&&existing.exists(){let old:Value=serde_json::from_slice(&fs::read(&existing).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;if old["jobId"]!=value["jobId"]||old["mode"]!=value["mode"]||old["publication"]!=value["publication"]||old["artifacts"]!=value["artifacts"]{return Err("既存manifestと再検証済み成果物が異なります".into())}value=old;}
  let text=serde_json::to_vec_pretty(&value).map_err(|e|e.to_string())?;
  self.write_artifact(n,n.output_artifacts.first().map(String::as_str).unwrap_or("manifest.json"),&text,"manifest")?;Ok(())
 }
 pub(crate) fn quality_node(&self,n:&Node)->Result<(),String>{
  let s=self.snapshot();let j=s.job.as_ref().ok_or("ジョブがありません")?;if crate::general::applies(j){crate::general::run_tests(self,j,n)?;}
  let s=self.snapshot();let j=s.job.as_ref().unwrap();let mut q=crate::practical::verify_product(j)?;
  let existing=Path::new(&j.workspace).join("quality.json");if j.recovery.is_some()&&existing.exists(){let old:crate::practical::Quality=serde_json::from_slice(&fs::read(&existing).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;if old.artifact_sha256!=q.artifact_sha256||old.machine_status!=q.machine_status||old.checks!=q.checks||old.display_status!="NOT_VERIFIED"{return Err("既存qualityと再検証が異なります".into())}q=old;}
  let bytes=if crate::decisions::enabled(j){
   if j.recovery.is_some()&&existing.exists(){fs::read(&existing).map_err(|e|e.to_string())?}else{let mut value=serde_json::to_value(&q).map_err(|e|e.to_string())?;value["decisionInspection"]=crate::decisions::inspection(j);serde_json::to_vec_pretty(&value).map_err(|e|e.to_string())?}
  }else{serde_json::to_vec_pretty(&q).map_err(|e|e.to_string())?};
  self.write_artifact(n,"quality.json",&bytes,"quality")?;
  self.update(|s|{s.job.as_mut().unwrap().quality=Some(q);s.event(Some(&n.node_id),"product_quality_checked","running","完成品の要件と実hashを照合。画面表示はNOT_VERIFIEDです。",None,vec![]);Ok(())})
 }
 pub(crate) fn accept_codex_output(&self,n:&Node,name:&str,output:&str)->Result<(),String>{
  let s=self.snapshot();let j=s.job.as_ref().ok_or("ジョブがありません")?;
  if !j.workflow.nodes.iter().any(|x|x.node_id==n.node_id&&x.output_artifacts.iter().any(|x|x==name)){return Err("回収対象のTask・出力名が一致しません".into())}
  if crate::sns_team::applies(j)&&n.node_id=="research"{crate::sns_team::validate_research(output)?;}
  if crate::sns::applies(j)&&n.node_id=="plan"{crate::sns::validate_plan(output)?;}
  let plan=if j.practical.is_some()&&n.node_id=="plan"{Some(if crate::general::applies(j){crate::general::parse_plan(&crate::decisions::production_payload(j,output),&j.instruction)?}else{crate::practical::parse_plan(output,&j.instruction)?})}else{None};
  if crate::general::applies(j)&&n.node_id=="writing"{let files=crate::general::inspect_output(j,output)?;for file in files{if let Some(a)=j.artifacts.iter().find(|a|a.node_id==n.node_id&&a.name==file.name){verify_artifact(a,Path::new(&j.workspace))?;if fs::read(&a.path).map_err(|e|e.to_string())?!=file.content.as_bytes(){return Err("既存の納品ファイルと正式応答が異なります".into())}}else{self.write_artifact(n,&file.name,file.content.as_bytes(),"product")?;}}}
  else if j.practical.is_some()&&n.node_id=="writing"{let p=j.practical.as_ref().and_then(|p|p.plan.as_ref()).ok_or("要件が未確定です")?;if j.workflow_id=="codex-document-v1"{crate::codex_only::inspect_versioned(output,p,&j.instruction,j.practical.as_ref().unwrap().version)?;}else{crate::practical::inspect_html(output,p,&j.instruction)?;}}
  if let Some(a)=j.artifacts.iter().find(|a|a.node_id==n.node_id&&a.name==name){verify_artifact(a,Path::new(&j.workspace))?;if fs::read_to_string(&a.path).map_err(|e|e.to_string())?!=self.scrub(output){return Err("回収済み本文と正式turn結果が一致しません。上書き・再送しません。".into())}}else{self.write_artifact(n,name,output.as_bytes(),"text")?;}
  self.update(|s|{let j=s.job.as_mut().unwrap();if let Some(p)=plan{j.practical.as_mut().unwrap().plan=Some(p);}j.nodes.get_mut(&n.node_id).unwrap().output=output.into();Ok(())})
 }
 fn manual_node(&self,n:&Node)->Result<(),String>{
  self.update(|s|{let j=s.job.as_mut().unwrap();j.status="awaiting_approval".into();j.nodes.get_mut(&n.node_id).unwrap().status="awaiting_approval".into();s.approvals.push(Approval{id:id(),request_id:Value::Null,job_id:j.job_id.clone(),node_id:n.node_id.clone(),kind:"manual_artifact".into(),title:"画像素材を登録してください".into(),details:n.work_instruction.clone(),params:json!({}),resolved:false,decision:None});s.event(Some(&n.node_id),"material_required","awaiting_approval","画像は自動生成していません。通常ChatGPT・Edgeで作成したPNG/JPEGの絶対パスを登録してください。",None,vec![]);Ok(())})?;
  loop{std::thread::sleep(Duration::from_millis(200));let s=self.snapshot();let j=s.job.as_ref().unwrap();if j.stop_requested{return Ok(())}if j.pause_requested{self.paused()?;return Ok(())}if j.status=="running"{return Ok(())}}
 }
 fn codex_node(&self,n:&Node)->Result<(),String>{
  self.require_secret_mask_for_recovery()?;
  let rpc=self.get_rpc()?;let(tx,rx)=mpsc::channel();*rpc.events.lock().unwrap()=Some(tx);
  let s=self.snapshot();let j=s.job.as_ref().unwrap();let thread=if let Some(t)=&j.thread_id{let loaded=rpc.call("thread/loaded/list",json!({}))?;if !loaded["data"].as_array().is_some_and(|ids|ids.iter().any(|id|id.as_str()==Some(t))){rpc.call("thread/resume",json!({"threadId":t,"excludeTurns":true}))?;}t.clone()}else{
   let mut thread_params=json!({"cwd":j.workspace,"approvalPolicy":"on-request","sandbox":"read-only","config":{"apps":{"_default":{"enabled":false}},"features":{"multi_agent":false}},"developerInstructions":"AI GUILDのローカル制作工程です。1つのCodex環境で順次処理します。読み取り専用で回答を作成してください。外部投稿・購入・削除・権限拡張・有料API・ChatGPT Work・画像生成は禁止。この製品の標準動作にブラウザ連携や別AIサービスは不要です。ファイル保存はホストが行います。未確認の事実・実行・画像目視を装わないでください。"});
   if s.settings.setup_complete&&!s.settings.model.is_empty(){thread_params["model"]=json!(s.settings.model);}
   let r=rpc.call("thread/start",thread_params)?;
   let t=r["thread"]["id"].as_str().ok_or("thread/start結果にIDがありません")?.to_string();self.update(|s|{s.job.as_mut().unwrap().thread_id=Some(t.clone());s.event(None,"thread_started","running",format!("このクエスト専用のCodexスレッドを開始しました。thread={t}; sandbox={}; approval={}",r["sandbox"],r["approvalPolicy"]),None,vec![]);Ok(())})?;t};
  self.update(|s|crate::presentation::bind_formal(s,&n.node_id,&thread,None))?;
  let sources=j.artifacts.iter().map(|a|format!("{} (SHA-256 {})",a.path,a.sha256)).collect::<Vec<_>>().join("\n");
  let additions=s.instructions.iter().filter(|i|i.job_id==j.job_id&&i.kind=="additional").map(|i|i.text.clone()).collect::<Vec<_>>().join("\n");
  let mut prompt_inputs=crate::practical::required_inputs(j,n)?;for a in j.artifacts.iter().filter(|a|a.kind=="input"){if !prompt_inputs.iter().any(|v|v.path==a.path){verify_artifact(a,Path::new(&j.workspace))?;prompt_inputs.push(a.clone())}}
  let content=prompt_inputs.iter().filter(|a|a.kind!="image").map(|a|fs::read_to_string(&a.path).map(|text|format!("\n--- {} ---\n{}",a.name,text)).map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?.join("\n");
  let prompt=format!("ユーザーの指示（原文）：\n{}\n\n追加指示：\n{}\n\n現在の工程：{} / {}\n作業：{}\n\n検証済みの前工程成果物：\n{}\n\n回収した前工程の全文：\n{}\n\nこの工程の完成した回答だけを出力してください。ホストがローカルへ保存します。",j.instruction,additions,n.role,n.label,n.work_instruction,sources,content);
  let current=self.snapshot();let current=current.job.as_ref().unwrap();if current.stop_requested||current.pause_requested{return Ok(())}
  self.update(|s|{if let Some(p)=s.job.as_mut().and_then(|j|j.practical.as_mut()){if p.turn_starts>=p.max_turn_starts{return Err(format!("{} turn上限に達しました。追加推論は実行しません。",p.max_turn_starts))}p.turn_starts+=1;}s.event(Some(&n.node_id),"turn_dispatch","running","turn/startの送信を開始。応答不明時は再送しません。",None,vec![]);Ok(())})?;
  let mut turn_params=json!({"threadId":thread,"input":[{"type":"text","text":prompt,"text_elements":[]}],"approvalPolicy":"on-request","sandboxPolicy":crate::rpc::readonly_policy()});
  if crate::sns::autonomous(j){if let Some(schema)=crate::sns_output::for_job(j,&n.node_id){turn_params["outputSchema"]=schema;}}
  let r=match rpc.call("turn/start",turn_params){Ok(r)=>r,Err(e)=>{self.unknown(&n.node_id,&e)?;return Ok(())}};
  let Some(turn)=r["turn"]["id"].as_str().filter(|v|!v.is_empty()).map(String::from)else{self.unknown(&n.node_id,"turn/start結果の正式IDが未確認です。再送しません。")?;return Ok(())};
  if let Err(error)=self.update(|s|{s.job.as_mut().unwrap().turn_id=Some(turn.clone());crate::presentation::bind_formal(s,&n.node_id,&thread,Some(&turn))?;s.event(Some(&n.node_id),"turn_started","running","正式turn/start応答をTaskに保存しました。",None,vec![]);Ok(())}){self.unknown(&n.node_id,&format!("正式turn対応の保存に失敗しました。再送しません: {error}"))?;return Ok(())}
  if self.snapshot().job.as_ref().is_some_and(|j|j.stop_requested||j.pause_requested){rpc.call("turn/interrupt",json!({"threadId":thread,"turnId":turn}))?;}
  let mut output=String::new();let mut last_output=std::time::Instant::now();let mut items=std::collections::HashMap::<String,Value>::new();
  loop{
   match rx.recv_timeout(Duration::from_millis(300)){
    Err(mpsc::RecvTimeoutError::Timeout)=>{if !rpc.alive.load(Ordering::SeqCst){self.unknown(&n.node_id,"stdio接続切断。元スレッドの状態を確認してください。")?;return Ok(())}continue},Err(_)=>{self.unknown(&n.node_id,"イベント受信が終了しました。")?;return Ok(())},Ok(v)=>{
     let method=v["method"].as_str().unwrap_or("");let p=&v["params"];
     if method=="guild/disconnected"{self.unknown(&n.node_id,"app-server切断。実行結果は未確認です。")?;return Ok(())}
     if v.get("id").is_some(){if p["threadId"]!=thread||p["turnId"]!=turn{rpc.send(json!({"id":v["id"],"error":{"code":-32600,"message":"Request does not match active thread/turn"}}))?;}else{self.server_request(n,&v,&rpc,p["itemId"].as_str().and_then(|id|items.get(id)))?;}continue}
     if let Some(t)=p["threadId"].as_str(){if t!=thread{continue}}
     if let Some(t)=p["turnId"].as_str(){if t!=turn{continue}}
     if method=="serverRequest/resolved"{self.resolve_request(&thread,&p["requestId"])?;continue}
     if method=="item/started"{if let Some(id)=p["item"]["id"].as_str(){items.insert(id.into(),p["item"].clone());}}
     if method=="item/agentMessage/delta"{if let Some(delta)=p["delta"].as_str(){output.push_str(delta);if last_output.elapsed()>Duration::from_millis(350)&&self.secrets.lock().unwrap().is_empty(){let current=output.clone();self.update(|s|{s.job.as_mut().unwrap().nodes.get_mut(&n.node_id).unwrap().output=current;Ok(())})?;last_output=std::time::Instant::now();}}}
     if method=="item/started"{let typ=p["item"]["type"].as_str().unwrap_or("item");if typ!="agentMessage"{self.update(|s|{s.event(Some(&n.node_id),"item_started","running",format!("Codex：{typ} を開始。"),None,vec![]);Ok(())})?;}}
     if method=="item/completed"&&p["item"]["type"]=="agentMessage"{if let Some(text)=p["item"]["text"].as_str(){output=text.to_string();self.update(|s|{s.job.as_mut().unwrap().nodes.get_mut(&n.node_id).unwrap().output=text.to_string();s.event(Some(&n.node_id),"agent_message_received","running","正式agentMessageを受信しました。保存・品質確認は別工程です。",None,vec![]);Ok(())})?;}}
     if method=="turn/completed"&&p["turn"]["id"]==turn{
      self.expire_turn_requests(&thread,&turn)?;
      let status=p["turn"]["status"].as_str().unwrap_or("unknown");
      self.update(|s|{let j=s.job.as_mut().unwrap();j.turn_id=None;s.event(Some(&n.node_id),"turn_completed",if status=="completed"{"running"}else if status=="interrupted"{"cancelled"}else{"failed"},format!("Codex正式結果：{status}"),None,vec![]);Ok(())})?;
      if status=="interrupted"{if self.snapshot().job.as_ref().unwrap().pause_requested{self.paused()?;return Ok(())}self.cancelled()?;return Ok(())}
      if status!="completed"{return Err(format!("Codexの正式結果は {status} でした。{}",p["turn"]["error"]["message"].as_str().unwrap_or("")))}
      if self.snapshot().job.as_ref().unwrap().pause_requested{self.paused()?;return Ok(())}
      if output.trim().is_empty(){return Err("正式な完了結果を受信しましたが、保存する本文がありません。".into())}
      let name=n.output_artifacts.first().cloned().unwrap_or_else(||format!("{}.md",n.node_id));
      self.accept_codex_output(n,&name,&output)?;return Ok(())
     }
    }
   }
  }
 }
 fn server_request(&self,n:&Node,v:&Value,rpc:&Rpc,item:Option<&Value>)->Result<(),String>{
  let _gate=self.response_gate.lock().map_err(|_|"回答ロックエラー")?;
  let method=v["method"].as_str().unwrap_or("");let allowed=["item/commandExecution/requestApproval","item/fileChange/requestApproval","item/tool/requestUserInput"];
  if !allowed.contains(&method){rpc.send(json!({"id":v["id"],"error":{"code":-32601,"message":"Unsupported request: operation not approved by AI GUILD"}}))?;self.update(|s|{s.event(Some(&n.node_id),"request_rejected","running",format!("未対応のサーバー要求を拒否：{method}"),None,vec![]);Ok(())})?;return Ok(())}
  let current=self.snapshot();let j=current.job.as_ref().ok_or("ジョブがありません")?;
  if current.approvals.iter().any(|a|a.job_id==j.job_id&&!a.resolved&&a.request_id==v["id"]){return Ok(())}
  if j.stop_requested||j.pause_requested||j.active_node_id.as_deref()!=Some(&n.node_id)||j.thread_id.as_deref()!=v["params"]["threadId"].as_str()||j.turn_id.as_deref()!=v["params"]["turnId"].as_str(){return rpc.send(json!({"id":v["id"],"error":{"code":-32600,"message":"Inactive or stopping request"}}))}
  if crate::general::applies(j)&&method.ends_with("requestApproval"){rpc.send(json!({"id":v["id"],"result":{"decision":"decline"}}))?;self.update(|s|{s.event(Some(&n.node_id),"permission_scope_rejected","running","この版の汎用Jobは読み取り専用です。追加コマンド権限・ファイル変更は許可せず、応答による納品を続けます。",None,vec![]);Ok(())})?;return Ok(())}
  if method=="item/tool/requestUserInput"&&crate::sns::autonomous(j){rpc.send(json!({"id":v["id"],"error":{"code":-32601,"message":"The user requested autonomous editorial decisions. Continue using supplied facts and reasonable editorial choices; omit unavailable personal experiences and never invent facts. Return the requested files without clarification questions."}}))?;self.update(|s|{s.event(Some(&n.node_id),"sns_clarification_not_requested","running","利用者指定に従い、確認待ちにせずAI判断で制作を続けます。体験・事実は創作しません。",None,vec![]);Ok(())})?;return Ok(())}
  if method=="item/tool/requestUserInput"{if let Err(reason)=rpc.validate_question_support(&v["params"]){rpc.send(json!({"id":v["id"],"error":{"code":-32601,"message":"Secret questions are unavailable in this AI GUILD version"}}))?;self.update(|s|{s.event(Some(&n.node_id),"secret_request_rejected","running",reason,None,vec![]);Ok(())})?;return Ok(())}}
  let mut params=v["params"].clone();if method=="item/fileChange/requestApproval"{if let Some(item)=item{params["proposedItem"]=item.clone();}}
  self.update(|s|{let j=s.job.as_mut().unwrap();j.status="awaiting_approval".into();j.nodes.get_mut(&n.node_id).unwrap().status="awaiting_approval".into();s.approvals.push(Approval{id:id(),request_id:v["id"].clone(),job_id:j.job_id.clone(),node_id:n.node_id.clone(),kind:method.into(),title:if method.ends_with("requestUserInput"){ "Codexからの確認" }else if method.contains("fileChange"){"ファイル変更の承認"}else{"コマンド実行の承認"}.into(),details:redact(&serde_json::to_string_pretty(&params).unwrap_or_default()),params:redact_json(&params),resolved:false,decision:None});s.event(Some(&n.node_id),"approval_requested","awaiting_approval","確認事項への回答を待っています。",None,vec![]);Ok(())})
 }
 pub(crate) fn scrub(&self,text:&str)->String{let mut text=text.to_string();for value in self.secrets.lock().unwrap().iter(){text=text.replace(value,"[秘密の回答は保存しません]");}text}
 pub(crate) fn require_secret_mask_for_recovery(&self)->Result<(),String>{let s=self.snapshot();if self.secrets.lock().unwrap().is_empty()&&s.job.as_ref().is_some_and(|j|s.approvals.iter().any(|a|a.job_id==j.job_id&&a.kind.ends_with("requestUserInput")&&a.decision.is_some()&&a.decision.as_deref()!=Some("expired_by_server")&&a.params["questions"].as_array().is_some_and(|qs|qs.iter().any(|q|q["isSecret"]==true)))){return Err("秘密回答の照合情報は再起動で消去されています。未マスクの本文を取得・保存しないため、この元スレッドの追加実行・本文回収を停止しました。元サーバーの保持状態を確認してください。".into())}Ok(())}
 fn resolve_request(&self,thread:&str,request:&Value)->Result<(),String>{
  let _gate=self.response_gate.lock().map_err(|_|"回答ロックエラー")?;
  self.update(|s|{let Some(j)=s.job.as_ref()else{return Ok(())};if j.thread_id.as_deref()!=Some(thread){return Ok(())}let job_id=j.job_id.clone();let Some(a)=s.approvals.iter_mut().find(|a|a.job_id==job_id&&!a.resolved&&a.request_id==*request)else{return Ok(())};a.resolved=true;let sent=a.decision.is_some();if !sent{a.decision=Some("expired_by_server".into());}let node=a.node_id.clone();if !s.approvals.iter().any(|a|a.job_id==job_id&&!a.resolved){let j=s.job.as_mut().unwrap();if j.status=="awaiting_approval"{j.status="running".into();j.nodes.get_mut(&node).unwrap().status="running".into();}}s.event(Some(&node),if sent{"approval_response_resolved"}else{"approval_expired"},"running",if sent{"サーバーが回答要求の解決を通知しました。操作の成功は正式結果を待ちます。"}else{"サーバーが確認要求を終了しました。回答は送信していません。"},None,vec![]);Ok(())})
 }
 fn expire_turn_requests(&self,thread:&str,turn:&str)->Result<(),String>{let ids=self.snapshot().approvals.iter().filter(|a|!a.resolved&&a.params["threadId"]==thread&&a.params["turnId"]==turn).map(|a|a.request_id.clone()).collect::<Vec<_>>();for id in ids{self.resolve_request(thread,&id)?;}Ok(())}
 pub fn answer(&self,approval_id:String,decision:String,text:String)->Result<(),String>{self.answer_structured(approval_id,decision,text,None)}
 pub fn answer_structured(&self,approval_id:String,decision:String,text:String,answers:Option<std::collections::HashMap<String,Vec<String>>>)->Result<(),String>{
  let _gate=self.response_gate.lock().map_err(|_|"回答ロックエラー")?;
  let s=self.snapshot();let a=s.approvals.iter().find(|a|a.id==approval_id&&!a.resolved&&a.decision.is_none()).cloned().ok_or("有効な未回答の確認事項がありません。送信済み・失効した要求は再送しません。")?;let j=s.job.as_ref().ok_or("ジョブがありません")?;
  if a.job_id!=j.job_id||j.status!="awaiting_approval"||j.stop_requested||j.pause_requested||j.active_node_id.as_deref()!=Some(&a.node_id){return Err("現在のジョブと確認事項が一致しません。".into())}
  let mut saved_text=if text.is_empty(){decision.clone()}else{text.clone()};
  if a.kind=="manual_artifact"{
   if decision!="accept"{return Err("素材を登録するか、停止してください。".into())}
   let p=Path::new(text.trim());if !p.is_absolute()||!p.is_file(){return Err("存在する画像ファイルの絶対パスが必要です。".into())}let bytes=fs::read(p).map_err(|e|e.to_string())?;
   if bytes.len()>32*1024*1024||!(bytes.starts_with(b"\x89PNG\r\n\x1a\n")||bytes.starts_with(&[0xff,0xd8,0xff])){return Err("32MB以下のPNGまたはJPEGを指定してください。".into())}
   inspect_image_bytes(&bytes)?;
   let n=j.workflow.nodes.iter().find(|n|n.node_id==a.node_id).unwrap();let ext=if bytes.starts_with(b"\x89PNG"){ "png" }else{"jpg"};self.write_artifact(n,&format!("image-{}.{}",id(),ext),&bytes,"image")?;
  }else{
   let rpc=self.get_rpc()?;let result=if a.kind.ends_with("requestUserInput"){
    rpc.validate_question_support(&a.params)?;
    let(result,summary)=build_user_answers(&a.params,&text,answers)?;saved_text=summary;
    if let Some(questions)=a.params["questions"].as_array(){let mut secrets=self.secrets.lock().unwrap();for q in questions.iter().filter(|q|q["isSecret"]==true){if let Some(values)=result["answers"][q["id"].as_str().unwrap_or("")]["answers"].as_array(){for value in values.iter().filter_map(Value::as_str){if !value.is_empty()&&!secrets.iter().any(|v|v==value){secrets.push(value.into());}}}}}result
   }else{if !["accept","decline"].contains(&decision.as_str()){return Err("この承認結果は利用できません。".into())}if decision=="accept"&&a.kind=="item/fileChange/requestApproval"&&!a.params["proposedItem"]["changes"].is_array(){return Err("変更内容を取得できていないため許可できません。拒否または停止できます。".into())}json!({"decision":decision})};
   if a.params["threadId"].as_str()!=j.thread_id.as_deref()||a.params["turnId"].as_str()!=j.turn_id.as_deref(){return Err("要求のthread/turnが現在の実行と一致しません。".into())}
   self.update(|s|{s.approvals.iter_mut().find(|x|x.id==a.id).unwrap().decision=Some("sending_unknown".into());Ok(())})?;
   if let Err(err)=rpc.send(json!({"id":a.request_id,"result":result})){self.unknown(&a.node_id,"回答の送信結果は未確認です。再送せず元の要求を照合してください。")?;return Err(err)}
  }
  self.update(|s|{let j=s.job.as_mut().unwrap();if a.kind=="manual_artifact"{j.status="running".into();j.nodes.get_mut(&a.node_id).unwrap().status="running".into();}let status=j.status.clone();let a=s.approvals.iter_mut().find(|a|a.id==approval_id).unwrap();a.resolved=a.kind=="manual_artifact";a.decision=Some(decision.clone());s.instructions.push(Instruction{id:id(),job_id:j.job_id.clone(),kind:"approval".into(),text:if a.kind=="manual_artifact"{"画像素材を登録".into()}else{saved_text},timestamp:now()});let node_id=a.node_id.clone();s.event(Some(&node_id),"approval_answered",&status,format!("確認への回答を書き込みました（サーバー解決待ち）：{decision}"),None,vec![]);Ok(())})
 }
 pub fn control(self:&Arc<Self>,action:String)->Result<(),String>{
  let s=self.snapshot();let j=s.job.as_ref().ok_or("クエストを開始してください")?;
  if j.collaboration.is_some(){return Err("画像付きJobは同じTaskの保存状態を照合してください。通常retry/resumeによる再推論は行いません。".into())}
  match action.as_str(){
   "reconcile"=>{
    if j.status!="disconnected"{return Err("状態不明のクエストだけを照合します。".into())}
    self.require_secret_mask_for_recovery()?;
    let rpc=self.get_rpc()?;
    let Some(thread)=&j.thread_id else{
     if !s.events.iter().any(|e|e.job_id==j.job_id&&e.event_type=="turn_dispatch"){return self.cancelled()}
     return Err("送信先IDが未確認です。元実行の特定前に再送・再試行はできません。".into())
    };
    rpc.call("thread/resume",json!({"threadId":thread,"excludeTurns":true}))?;
    let turns=rpc.call("thread/turns/list",json!({"threadId":thread,"limit":100,"sortDirection":"desc","itemsView":"summary"}))?;
    let known=j.turn_id.as_deref().ok_or("元turn IDが未取得です。既存結果を特定するまで再送しません。")?;
    let turn=turns["data"].as_array().and_then(|a|a.iter().find(|t|t["id"]==known)).ok_or("元turnを取得できません。結果は引き続き未確認です。")?;
    match turn["status"].as_str().unwrap_or("unknown"){
     "completed"=>{
      let entries=rpc.call("thread/items/list",json!({"threadId":thread,"turnId":known,"limit":100,"sortDirection":"desc"}))?;
      let output=entries["data"].as_array().and_then(|rows|rows.iter().find(|e|e["item"]["type"]=="agentMessage"&&e["item"]["phase"].as_str()!=Some("commentary"))).and_then(|e|e["item"]["text"].as_str()).ok_or("元turnの完成した本文を取得できません。")?;
      let node_id=j.active_node_id.as_ref().ok_or("元工程を特定できません")?;let n=j.workflow.nodes.iter().find(|n|&n.node_id==node_id).ok_or("元工程がありません")?;
      let name=n.output_artifacts.first().ok_or("元出力名がありません")?;
      self.accept_codex_output(n,name,output)?;
      self.update(|s|{let j=s.job.as_mut().unwrap();j.status="paused".into();j.turn_id=None;let r=j.nodes.get_mut(node_id).unwrap();r.status="paused".into();r.output=redact(output);r.ended_at=Some(now());s.event(Some(node_id),"recovered_formal_completion","paused","元turnの正式な完了と本文を再取得しました。再送せず、回収済み工程として再開できます。",None,vec![]);Ok(())})?;
      // A recovered completed stage is promoted only after its actual file was checked.
      self.update(|s|{let j=s.job.as_mut().unwrap();j.nodes.get_mut(node_id).unwrap().status="running".into();Ok(())})?;
      self.update(|s|{s.job.as_mut().unwrap().nodes.get_mut(node_id).unwrap().status="completed".into();Ok(())})
     },
     "interrupted"=>self.cancelled(),
     "failed"=>self.update(|s|{let j=s.job.as_mut().unwrap();j.status="failed".into();j.ended_at=Some(now());if let Some(id)=&j.active_node_id{j.nodes.get_mut(id).unwrap().status="failed".into();}s.event(None,"recovered_formal_failure","failed","元turnの正式な失敗を確認しました。再試行できます。",None,vec![]);Ok(())}),
     status=>{self.update(|s|{s.event(None,"recovery_still_unknown","disconnected",format!("元turnの現在状態：{status}。新しいturnは送信していません。"),None,vec![]);Ok(())})?;Err("元の実行が継続中、または結果未確定です。時間をおいて照合するか、元の実行を中断してください。".into())}
    }
   },
   "interrupt_unknown"=>{
    if j.status!="disconnected"{return Err("状態不明ではありません。".into())}
    let rpc=self.get_rpc()?;let thread=j.thread_id.as_ref().ok_or("元thread IDがありません")?;let turn=j.turn_id.as_ref().ok_or("元turn IDがありません")?;
    let(tx,rx)=mpsc::channel();*rpc.events.lock().unwrap()=Some(tx);rpc.call("thread/resume",json!({"threadId":thread,"excludeTurns":true}))?;rpc.call("turn/interrupt",json!({"threadId":thread,"turnId":turn}))?;
    let until=std::time::Instant::now()+Duration::from_secs(45);
    while std::time::Instant::now()<until{if let Ok(v)=rx.recv_timeout(Duration::from_millis(300)){if v["method"]=="turn/completed"&&v["params"]["threadId"]==*thread&&v["params"]["turn"]["id"]==*turn&&v["params"]["turn"]["status"]=="interrupted"{return self.cancelled()}}}
    Err("中断要求後の正式結果は未確認です。再送せず、元スレッドを照合してください。".into())
   },
   "stop"|"pause"=>{
    let _gate=self.response_gate.lock().map_err(|_|"回答ロックエラー")?;
    if action=="stop"&&crate::sns::applies(j)&&j.active_node_id.as_deref()==Some(crate::sns::REVIEW)&&!self.worker.load(Ordering::SeqCst){return self.cancelled()}
    if action=="stop"&&j.status=="paused"{return self.cancelled()}
    if !["running","awaiting_approval","handing_off"].contains(&j.status.as_str()){return Err("現在は実行中ではありません。".into())}
    self.update(|s|{let j=s.job.as_mut().unwrap();if action=="pause"{j.pause_requested=true;}else{j.stop_requested=true;}s.event(None,if action=="pause"{"pause_requested"}else{"stop_requested"},"running","中断を要求しました。正式な中断結果、またはローカル工程の停止を待ちます。",None,vec![]);Ok(())})?;
    if j.mode=="real"{if let(Some(thread),Some(turn))=(&j.thread_id,&j.turn_id){if let Err(err)=self.get_rpc()?.call("turn/interrupt",json!({"threadId":thread,"turnId":turn})){self.unknown(j.active_node_id.as_deref().unwrap_or(""),&err)?;return Err(err)}}}
    Ok(())
   },
   "retry"|"resume"=>{
    if j.recovery.is_some(){return Err("このJobは正式応答からのホスト復旧専用です。新しい推論を再試行しません。".into())}
    if !["failed","cancelled","paused"].contains(&j.status.as_str()){return Err("再試行は失敗・停止・一時停止した工程で利用できます。状態不明では再送しません。".into())}
    if self.worker.swap(true,Ordering::SeqCst){return Err("前の実行の終了確認中です。少し待ってください。".into())}
    if j.mode=="real"{if let Err(e)=self.get_rpc(){self.worker.store(false,Ordering::SeqCst);return Err(e)}}
    let r=self.update(|s|{let j=s.job.as_mut().unwrap();j.stop_requested=false;j.pause_requested=false;j.status="queued".into();j.ended_at=None;j.turn_id=None;for n in j.nodes.values_mut(){if ["failed","cancelled","paused"].contains(&n.status.as_str()){n.status="queued".into();n.error=None;}}for a in s.approvals.iter_mut().filter(|a|a.job_id==j.job_id&&!a.resolved){a.resolved=true;a.decision=Some("superseded_after_stop".into());}s.event(None,"retry_requested","queued","未完了の工程だけを再試行。既存成果物と履歴を保持します。",None,vec![]);Ok(())});
    if let Err(e)=r{self.worker.store(false,Ordering::SeqCst);return Err(e)}self.spawn_worker();Ok(())
   },
   _=>Err("未対応の操作です。".into())
  }
 }
 pub fn answer_content_review(self:&Arc<Self>,job_id:&str,approval_id:&str,decision:&str)->Result<(),String>{
  let _gate=self.response_gate.lock().map_err(|_|"回答ロックエラー")?;
  if self.worker.swap(true,Ordering::SeqCst){return Err("前の工程の終了確認中です。少し待ってください".into())}
  let result=crate::sns::answer(self,job_id,approval_id,decision);
  match result{Ok(()) if decision=="accept"=>{self.spawn_worker();Ok(())},other=>{self.worker.store(false,Ordering::SeqCst);other}}
 }
 pub fn additional(&self,text:String)->Result<(),String>{
  if text.trim().is_empty()||text.len()>64000{return Err("追加指示を入力してください。".into())}
  let s=self.snapshot();let j=s.job.as_ref().ok_or("クエストがありません")?;
  if j.status=="disconnected"{return Err("状態不明のため送信しません。元スレッドを確認してください。".into())}
  if let(Some(thread),Some(turn))=(&j.thread_id,&j.turn_id){self.get_rpc()?.call("turn/steer",json!({"threadId":thread,"expectedTurnId":turn,"input":[{"type":"text","text":text,"text_elements":[]}]}))?;}
  self.update(|s|{let j=s.job.as_ref().unwrap();s.instructions.push(Instruction{id:id(),job_id:j.job_id.clone(),kind:"additional".into(),text,timestamp:now()});s.event(None,"additional_instruction",&j.status.clone(),"追加指示を保存しました。実行中はturn/steer、それ以外は次の工程に適用します。",None,vec![]);Ok(())})
 }
 pub fn cancelled(&self)->Result<(),String>{self.update(|s|{let j=s.job.as_mut().unwrap();j.status="cancelled".into();j.stop_requested=false;j.ended_at=Some(now());j.turn_id=None;for n in j.nodes.values_mut(){if !["completed","failed"].contains(&n.status.as_str()){n.status="cancelled".into();n.ended_at=Some(now());}}for a in s.approvals.iter_mut().filter(|a|a.job_id==j.job_id&&!a.resolved){a.resolved=true;a.decision=Some("cancelled".into());}s.event(None,"job_cancelled","cancelled","実行の停止を確認しました。途中の成果物と履歴は保持しています。",None,vec![]);Ok(())})}
 fn paused(&self)->Result<(),String>{self.update(|s|{let j=s.job.as_mut().unwrap();j.status="paused".into();j.pause_requested=false;j.turn_id=None;if let Some(id)=&j.active_node_id{let r=j.nodes.get_mut(id).unwrap();r.status="paused".into();r.ended_at=Some(now());}s.event(None,"job_paused","paused","実行を中断し、一時停止しました。再開は未完了工程の新しい試行になります。",None,vec![]);Ok(())})}
 fn unknown(&self,node:&str,message:&str)->Result<(),String>{self.update(|s|{s.connection.status="disconnected".into();s.connection.error=Some(message.into());let j=s.job.as_mut().unwrap();j.status="disconnected".into();if let Some(n)=j.nodes.get_mut(node){n.status="disconnected".into();}s.event(Some(node),"connection_lost","disconnected",message,None,vec![]);Ok(())})}

}
pub fn artifact(p:&Path,node:&str,kind:&str)->Result<Artifact,String>{let bytes=fs::read(p).map_err(|e|e.to_string())?;Ok(Artifact{path:p.canonicalize().map_err(|e|e.to_string())?.to_string_lossy().trim_start_matches("\\\\?\\").into(),name:p.file_name().unwrap_or_default().to_string_lossy().into(),node_id:node.into(),sha256:format!("{:x}",Sha256::digest(&bytes)),bytes:bytes.len() as u64,kind:kind.into()})}
pub fn verify_artifact(a:&Artifact,workspace:&Path)->Result<(),String>{let p=Path::new(&a.path).canonicalize().map_err(|_|format!("成果物がありません: {}",a.name))?;let root=workspace.canonicalize().map_err(|e|e.to_string())?;if !p.starts_with(&root){return Err("ジョブの保存範囲外の成果物です。".into())}let actual=artifact(&p,&a.node_id,&a.kind)?;if actual.sha256!=a.sha256||actual.bytes!=a.bytes{return Err(format!("成果物のhashが変わっています: {}",a.name))}Ok(())}
pub fn inspect_image_bytes(bytes:&[u8])->Result<(u32,u32),String>{
 if bytes.len()>32*1024*1024||!(bytes.starts_with(b"\x89PNG\r\n\x1a\n")||bytes.starts_with(&[0xff,0xd8,0xff])){return Err("32MB以下のPNG/JPEGが必要です".into())}
 let mut reader=image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|e|e.to_string())?;
 let mut limits=image::Limits::default();limits.max_image_width=Some(8192);limits.max_image_height=Some(8192);limits.max_alloc=Some(96*1024*1024);reader.limits(limits);
 let decoded=reader.decode().map_err(|e|format!("画像を完全にdecodeできません: {e}"))?;Ok((decoded.width(),decoded.height()))
}
pub fn safe_project(path:String)->Result<PathBuf,String>{let p=PathBuf::from(path);if !p.is_absolute()||!p.is_dir(){return Err("既存フォルダの絶対パスを指定してください。".into())}p.canonicalize().map_err(|e|e.to_string())}
pub(crate) fn job_workspace(root:&Path,job_id:&str)->Result<PathBuf,String>{
 if job_id.is_empty()||!job_id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'){return Err("Job IDが不正です".into())}
 let root=root.canonicalize().map_err(|e|e.to_string())?;let quests=root.join("quests");
 if let Ok(m)=fs::symlink_metadata(&quests){
  #[cfg(windows)]{use std::os::windows::fs::MetadataExt;if m.file_attributes()&0x400!=0{return Err("納品先のquestsがreparse pointです。通常のフォルダを選択してください".into())}}
  #[cfg(not(windows))]if m.file_type().is_symlink(){return Err("納品先のquestsがsymlinkです".into())}
  if !m.is_dir(){return Err("納品先のquestsがフォルダではありません".into())}
 }else{fs::create_dir(&quests).map_err(|e|e.to_string())?;}
 let workspace=quests.join(job_id);fs::create_dir(&workspace).map_err(|e|format!("新規Jobフォルダを作れません（既存Jobは上書きしません）: {e}"))?;Ok(workspace)
}
fn redact_json(v:&Value)->Value{match v{Value::String(s)=>Value::String(redact(s)),Value::Array(a)=>Value::Array(a.iter().map(redact_json).collect()),Value::Object(o)=>Value::Object(o.iter().map(|(k,v)|{let key=k.to_ascii_lowercase();(k.clone(),if v.is_string()&&["token","password","secret","authorization","cookie","api_key"].iter().any(|w|key.contains(w)){json!("[REDACTED]")}else{redact_json(v)})}).collect()),_=>v.clone()}}
fn scrub_text(text:&mut String,secrets:&[String]){for secret in secrets{*text=text.replace(secret,"[秘密の回答は保存しません]");}}
fn scrub_values(value:&mut Value,secrets:&[String]){match value{Value::String(text)=>scrub_text(text,secrets),Value::Array(rows)=>for row in rows{scrub_values(row,secrets)},Value::Object(rows)=>for(key,row)in rows{if !["id","threadId","turnId","itemId","requestId"].contains(&key.as_str()){scrub_values(row,secrets)}},_=>{}}}
fn scrub_snapshot(s:&mut Snapshot,secrets:&[String]){if secrets.is_empty(){return}for j in s.job.iter_mut().chain(s.history.iter_mut()){scrub_text(&mut j.instruction,secrets);for r in j.nodes.values_mut(){scrub_text(&mut r.output,secrets);if let Some(error)=&mut r.error{scrub_text(error,secrets)}}}for event in &mut s.events{scrub_text(&mut event.message,secrets)}for instruction in &mut s.instructions{scrub_text(&mut instruction.text,secrets)}for approval in &mut s.approvals{scrub_text(&mut approval.details,secrets);scrub_values(&mut approval.params,secrets)}if let Some(error)=&mut s.connection.error{scrub_text(error,secrets)}}
pub fn build_user_answers(params:&Value,text:&str,answers:Option<std::collections::HashMap<String,Vec<String>>>)->Result<(Value,String),String>{
 let questions=params["questions"].as_array().filter(|q|!q.is_empty()).ok_or("質問形式が不正です")?;
 let supplied=match answers{Some(a)=>a,None=>{if questions.len()!=1{return Err("複数の質問には質問ごとの回答が必要です。同じ回答を自動複製しません。".into())}std::collections::HashMap::from([(questions[0]["id"].as_str().ok_or("質問IDがありません")?.to_string(),vec![text.to_string()])])}};
 let mut result=serde_json::Map::new();let mut summary=vec![];
 for q in questions{let key=q["id"].as_str().filter(|k|!k.is_empty()).ok_or("質問IDがありません")?;if result.contains_key(key){return Err("質問IDが重複しています".into())}
  let answer=supplied.get(key).filter(|a|!a.is_empty()&&a.iter().all(|s|!s.trim().is_empty())).ok_or("すべての質問に個別の回答が必要です")?;
  result.insert(key.into(),json!({"answers":answer}));summary.push(format!("{}: {}",q["question"].as_str().unwrap_or(key),if q["isSecret"]==true{"[秘密の回答は保存しません]".into()}else{redact(&answer.join(" / "))}));
 }
 if supplied.len()!=result.len(){return Err("元の質問と回答IDが一致しません".into())}Ok((json!({"answers":result}),summary.join("\n")))
}


