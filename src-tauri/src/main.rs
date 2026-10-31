#![cfg_attr(not(debug_assertions), windows_subsystem="windows")]
mod presentation;#[cfg(test)]mod presentation_tests;
mod skin_package;mod skins;#[cfg(test)]mod skin_tests;
mod core_api;mod sns;mod browser;mod integrations;mod purposes;
mod sns_output;mod sns_team;
mod decisions;
#[cfg(test)]mod decision_tests;
#[cfg(test)]mod sns_team_tests;
#[cfg(test)]mod sns_output_tests;
mod posting;mod posting_wire;
#[cfg(test)]mod posting_tests;
mod providers;mod provider_credentials;mod provider_http;mod provider_buffer;
mod connectors;mod connector_mcp;mod connector_http;mod connector_webhook;
#[cfg(test)]mod connector_tests;
#[cfg(test)]mod provider_tests;
#[cfg(test)]mod integration_tests;
#[cfg(test)]mod browser_tests;
#[cfg(test)]mod open_core_tests;
#[cfg(test)]mod purpose_tests;
mod model;mod storage;mod rpc;mod engine;
mod qa_path;
mod practical;
mod codex_only;
mod setup;
mod general;
#[cfg(test)]mod productization_tests;
mod artifact_open;
#[cfg(test)]mod next_stage_tests;
#[cfg(test)]mod post_document_tests;
mod s2_core;
#[cfg(test)]mod s2_core_tests;
mod html_policy;
mod css_policy;
mod recovery;
mod plan_recovery;
#[cfg(test)]mod phase9_tests;
#[cfg(feature="qa-harness")]mod qa_live;
#[cfg(test)]mod tests;
#[cfg(test)]mod phase7_tests;
use std::sync::Arc;
use engine::Engine;
use model::{Snapshot,Workflow,Project};
use tauri::{Manager,State};
#[tauri::command] async fn skin_action(library:State<'_,Arc<skins::Library>>,request:skins::Request)->Result<serde_json::Value,String>{let library=library.inner().clone();tauri::async_runtime::spawn_blocking(move||library.perform(request)).await.map_err(|_|"SKIN_WORKER".to_string())?}
#[tauri::command] fn core_presentation_snapshot(e:State<'_,Arc<Engine>>,job_id:String)->Result<serde_json::Value,String>{e.presentation_snapshot(&job_id)}
#[tauri::command] fn core_snapshot(e:State<'_,Arc<Engine>>)->serde_json::Value{core_api::snapshot(&e.snapshot())}
#[tauri::command] async fn core_action(e:State<'_,Arc<Engine>>,request:core_api::Action)->Result<serde_json::Value,String>{let e=e.inner().clone();tauri::async_runtime::spawn_blocking(move||core_api::perform(&e,request)).await.map_err(|e|e.to_string())?}
#[tauri::command] fn get_snapshot(e:State<'_,Arc<Engine>>)->Snapshot{e.snapshot()}
#[tauri::command] async fn inspect_codex(path:String)->Result<serde_json::Value,String>{tauri::async_runtime::spawn_blocking(move||setup::inspect(&path)).await.map_err(|e|e.to_string())}
#[tauri::command] fn save_setup(e:State<'_,Arc<Engine>>,settings:setup::Settings)->Result<(),String>{e.save_setup(settings)}
#[tauri::command] async fn connection_details(e:State<'_,Arc<Engine>>)->Result<serde_json::Value,String>{let e=e.inner().clone();tauri::async_runtime::spawn_blocking(move||e.connection_details()).await.map_err(|e|e.to_string())?}
#[tauri::command] async fn connect_codex(e:State<'_,Arc<Engine>>)->Result<(),String>{let e=e.inner().clone();tauri::async_runtime::spawn_blocking(move||e.connect()).await.map_err(|x|x.to_string())?}
#[tauri::command] fn start_job(e:State<'_,Arc<Engine>>,instruction:String)->Result<String,String>{e.inner().start(instruction)}
#[tauri::command] async fn control_job(e:State<'_,Arc<Engine>>,action:String)->Result<(),String>{let e=e.inner().clone();tauri::async_runtime::spawn_blocking(move||e.control(action)).await.map_err(|x|x.to_string())?}
#[tauri::command] async fn send_instruction(e:State<'_,Arc<Engine>>,text:String)->Result<(),String>{let e=e.inner().clone();tauri::async_runtime::spawn_blocking(move||e.additional(text)).await.map_err(|x|x.to_string())?}
#[tauri::command] async fn answer_approval(e:State<'_,Arc<Engine>>,approval_id:String,decision:String,text:String,answers:Option<std::collections::HashMap<String,Vec<String>>>)->Result<(),String>{let e=e.inner().clone();tauri::async_runtime::spawn_blocking(move||e.answer_structured(approval_id,decision,text,answers)).await.map_err(|x|x.to_string())?}
#[tauri::command] fn save_workflow(e:State<'_,Arc<Engine>>,workflow:Workflow)->Result<(),String>{model::validate(&workflow)?;if e.worker.load(std::sync::atomic::Ordering::SeqCst){return Err("実行中はワークフローを変更できません。".into())}e.update(|s|{s.workflow=workflow;s.positions.clear();Ok(())})}
#[tauri::command] fn set_position(e:State<'_,Arc<Engine>>,node_id:String,x:f64,y:f64)->Result<(),String>{if !x.is_finite()||!y.is_finite()||x<0.||y<0.||x>20000.||y>20000.{return Err("座標が範囲外です".into())}e.update(|s|{if !s.workflow.nodes.iter().any(|n|n.node_id==node_id){return Err("工程がありません".into())}s.positions.insert(node_id,serde_json::json!({"x":x,"y":y}));Ok(())})}
#[tauri::command] fn reset_layout(e:State<'_,Arc<Engine>>)->Result<(),String>{e.update(|s|{s.positions.clear();Ok(())})}
#[tauri::command] fn set_mode(e:State<'_,Arc<Engine>>,mode:String)->Result<(),String>{if !["real","demo"].contains(&mode.as_str()){return Err("実行モードが不正です".into())}if e.worker.load(std::sync::atomic::Ordering::SeqCst){return Err("実行中はモードを変更できません。".into())}e.update(|s|{if s.job.as_ref().is_some_and(|j|["paused","disconnected","awaiting_approval"].contains(&j.status.as_str())){return Err("現在のクエストを終了してから切り替えてください。".into())}s.mode=mode;Ok(())})}
#[tauri::command] fn save_project(e:State<'_,Arc<Engine>>,name:String,path:String)->Result<(),String>{if e.worker.load(std::sync::atomic::Ordering::SeqCst){return Err("実行中はプロジェクトを変更できません。".into())}if name.trim().is_empty(){return Err("プロジェクト名を入力してください".into())}let p=engine::safe_project(path)?;e.update(|s|{let project=Project{name:name.trim().into(),path:p.to_string_lossy().trim_start_matches("\\\\?\\").into()};if !s.projects.iter().any(|p|p.path==project.path){s.projects.push(project.clone())}s.project=project;if s.settings.setup_complete{s.settings.work_root=s.project.path.clone();s.settings.input_paths.clear();}Ok(())})}
#[tauri::command] fn open_artifact(e:State<'_,Arc<Engine>>,path:String)->Result<(),String>{e.open_artifact(path)}
#[tauri::command] fn product_availability(e:State<'_,Arc<Engine>>,job_id:String)->serde_json::Value{let s=e.snapshot();let result=s.job.iter().chain(s.history.iter()).find(|j|j.job_id==job_id).ok_or("Job missing".to_string()).and_then(practical::verify_product);serde_json::json!({"available":result.is_ok(),"error":result.err()})}
#[tauri::command] fn recover_local_product(e:State<'_,Arc<Engine>>,identity:s2_core::Identity)->Result<(),String>{e.recover_local_core(identity)}
#[tauri::command] async fn recover_formal(e:State<'_,Arc<Engine>>,proof_path:String)->Result<(),String>{let e=e.inner().clone();tauri::async_runtime::spawn_blocking(move||e.recover_formal(proof_path)).await.map_err(|x|x.to_string())?}
#[tauri::command] fn practical_readiness(instruction:String)->Result<serde_json::Value,String>{let p=general::intake(&instruction)?.ok_or("仕事を入力してください")?;Ok(serde_json::json!({"workflow":practical::workflow_for(&p),"maxTurnStarts":p.max_turn_starts,"inferencePerformed":false,"displayVerification":"NOT_VERIFIED","externalSend":false}))}
fn main(){
 let plan_args=std::env::args().collect::<Vec<_>>();if plan_args.get(1).is_some_and(|s|s=="--recover-plan-formal"){if let Err(e)=plan_recovery::cli(&plan_args){eprintln!("{e}");std::process::exit(1)}return}
 let local_args=std::env::args().collect::<Vec<_>>();if local_args.get(1).is_some_and(|s|s=="--local-product"){if let Err(e)=s2_core::cli(&local_args){eprintln!("{e}");std::process::exit(1)}return}
 let args=std::env::args().collect::<Vec<_>>();if args.get(1).is_some_and(|s|s=="--recover-formal"){if let Err(e)=recovery::cli(&args){eprintln!("{e}");std::process::exit(1)}return}
 let continue_args=if plan_args.get(1).is_some_and(|s|s=="--continue-recovered-plan"){if plan_args.len()!=4{eprintln!("Usage: --continue-recovered-plan EXISTING_DATA GRANT");std::process::exit(1)}Some((plan_args[2].clone(),plan_args[3].clone()))}else{None};
 let app=tauri::Builder::default().setup(move|app|{let path=qa_path::select_data_path(app.path().app_local_data_dir()?,std::env::var_os("AI_GUILD_QA_DATA_DIR")).map_err(std::io::Error::other)?;if let Some((expected,_))=&continue_args{if path.canonicalize()?!=std::path::Path::new(expected).canonicalize()?{return Err(std::io::Error::other("元の復旧DBが一致しません").into())}}let e=Engine::open(&path,Some(app.handle().clone())).map_err(std::io::Error::other)?;app.manage(e.clone());app.manage(Arc::new(skins::Library::new(&path)));if let Some((_,grant))=&continue_args{let engine=e.clone();let grant=grant.clone();tauri::async_runtime::spawn_blocking(move||{if let Err(err)=plan_recovery::continue_writing(&engine,&grant){let _=engine.update(|s|{s.event(Some("writing"),"planned_continuation_error","failed",err,None,vec![]);Ok(())});}});}Ok(())})
 .invoke_handler(tauri::generate_handler![skin_action,core_presentation_snapshot,core_snapshot,core_action,get_snapshot,inspect_codex,save_setup,connection_details,connect_codex,start_job,control_job,send_instruction,answer_approval,save_workflow,set_position,reset_layout,set_mode,save_project,open_artifact,practical_readiness,recover_formal,recover_local_product,product_availability]).build(tauri::generate_context!()).expect("AI GUILDの起動に失敗しました");
 app.run(|handle,event|{if let tauri::RunEvent::Exit=event{let e=handle.state::<Arc<Engine>>();if let Ok(r)=e.rpc.lock(){if let Some(r)=r.as_ref(){r.shutdown();}};}});
}

