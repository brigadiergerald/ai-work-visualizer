use crate::{engine::*,model::*,practical::*};
use std::{fs,path::PathBuf};
const INPUT:&str="試験用の架空の読書会。2026年11月14日 14:00〜16:00、森の図書室、当日受付。日本語の開催案内と運営チェックリストを1つのHTMLにする。外部調査・公開・個人情報処理なし。";
fn plan()->Plan{Plan{production_constraints:vec![],title:"架空の読書会".into(),requirements:vec!["2026年11月14日".into(),"14:00〜16:00".into(),"森の図書室".into(),"当日受付".into(),"試験用の架空の読書会".into()],checklist:vec!["受付を準備する".into(),"会場を整える".into(),"終了後に片付ける".into()]}}
fn html()->String{let p=plan();format!("<!doctype html><html lang=\"ja\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>架空の読書会</title><style>body{{background:#fff;color:#152a34;font-family:sans-serif;margin:3rem;line-height:1.7}}section{{margin:2rem 0}}</style></head><body><h1>架空の読書会</h1><p>OFFLINE FIXTURE — AI推論による完成品ではありません</p><section><h2>開催案内</h2><p>{}</p></section><section><h2>運営チェックリスト</h2><ul>{}</ul></section></body></html>",p.requirements.join(" · "),p.checklist.iter().map(|x|format!("<li>{x}</li>")).collect::<String>())}
fn root()->PathBuf{let p=std::env::var_os("AI_GUILD_TEST_ROOT").map(PathBuf::from).expect("Phase9 tests require isolated test root").join(format!("practical-{}",id()));fs::create_dir_all(&p).unwrap();fs::write(p.join(".ai-guild-qa"),"offline fixtures only").unwrap();p}
fn fixture()->(std::sync::Arc<Engine>,PathBuf){let p=root();let e=Engine::open(&p,None).unwrap();let w=workflow();let j=Job{job_id:id(),workflow_id:w.workflow_id.clone(),nodes:w.nodes.iter().map(|n|(n.node_id.clone(),Run::default())).collect(),workflow:w,status:"running".into(),mode:"real".into(),instruction:INPUT.into(),started_at:now(),ended_at:None,workspace:p.to_string_lossy().into(),thread_id:Some("OFFLINE_THREAD".into()),turn_id:None,active_node_id:Some("plan".into()),artifacts:vec![],stop_requested:false,pause_requested:false,practical:intake(INPUT).unwrap(),quality:None,recovery:None,collaboration:None};e.update(|s|{s.job=Some(j);Ok(())}).unwrap();(e,p)}
#[test]fn phase9_formal_offline_pipeline_and_content_ng_stop(){
 use std::{time::{Instant,Duration},sync::atomic::Ordering};
 for ng in [false,true]{
  let p=root();fs::write(p.join("offline-plan.json"),serde_json::to_vec(&plan()).unwrap()).unwrap();fs::write(p.join("offline-event.html"),if ng{html().replace("当日受付","条件欠落")}else{html()}).unwrap();
  let e=Engine::open(&p,None).unwrap();let r=crate::rpc::Rpc::phase9_fixture(&p).unwrap();*e.rpc.lock().unwrap()=Some(r.clone());e.update(|s|{s.connection.version="MOCK / Phase9 offline fixture / NO INFERENCE".into();Ok(())}).unwrap();
  e.start(INPUT.into()).unwrap();let end=Instant::now()+Duration::from_secs(10);while e.worker.load(Ordering::SeqCst)&&Instant::now()<end{std::thread::sleep(Duration::from_millis(30))}assert!(!e.worker.load(Ordering::SeqCst));
  let s=e.snapshot();let j=s.job.as_ref().unwrap();let rows:Vec<serde_json::Value>=fs::read_to_string(p.join("offline-peer.jsonl")).unwrap().lines().map(|x|serde_json::from_str(x).unwrap()).collect();assert_eq!(rows.len(),2);assert!(rows.iter().all(|x|x["fullPlanReceived"]==true));assert_eq!(j.practical.as_ref().unwrap().turn_starts,2);
  if ng{assert_eq!(j.status,"failed");assert!(j.quality.is_none());assert_eq!(j.nodes["qa"].attempt,0);assert_eq!(j.artifacts.len(),1);assert!(!s.events.iter().any(|x|x.event_type=="handoff_received"&&x.node_id.as_deref()==Some("qa")));}
  else{assert_eq!(j.status,"completed");assert_eq!(j.artifacts.len(),4);let q=verify_product(j).unwrap();assert_eq!(q.artifact_sha256,j.quality.as_ref().unwrap().artifact_sha256);assert_eq!(j.quality.as_ref().unwrap().display_status,"NOT_VERIFIED");}
  fs::write(p.join("pipeline-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"classification":"OFFLINE_VERIFIED","contentNg":ng,"status":j.status,"jobId":j.job_id,"threadId":j.thread_id,"turns":rows,"artifactCount":j.artifacts.len(),"realInference":0,"realE2e":"NOT_RUN"})).unwrap()).unwrap();r.shutdown();drop(e);
  let e=Engine::open(&p,None).unwrap();let restored=e.snapshot();assert_eq!(restored.job.as_ref().unwrap().job_id,j.job_id);if !ng{verify_product(restored.job.as_ref().unwrap()).unwrap();}
 }
}
#[test]fn phase9_intake_and_plan_preserve_independent_locks(){
 assert!(intake(&INPUT.replace("当日受付","")).is_err());
 assert!(intake(INPUT).unwrap().is_some());assert!(intake("明るい挿絵1点を含むHTML").is_err());assert_eq!(intake("通常文章").unwrap().unwrap().version,4);
 validate(&workflow()).unwrap();let mut p=plan();let raw=serde_json::to_string(&p).unwrap();parse_plan(&raw,INPUT).unwrap();p.requirements.retain(|x|!x.contains("当日受付"));assert!(parse_plan(&serde_json::to_string(&p).unwrap(),INPUT).is_err());assert!(parse_plan("```json\n{}\n```",INPUT).is_err());
}
#[test]fn phase9_open_html_requires_actual_hash_bound_quality(){
 let(e,_)=fixture();let ns=e.snapshot().job.unwrap().workflow.nodes;e.accept_codex_output(&ns[0],"plan.json",&serde_json::to_string(&plan()).unwrap()).unwrap();e.accept_codex_output(&ns[1],"event.html",&html()).unwrap();let a=e.snapshot().job.unwrap().artifacts.into_iter().find(|a|a.name=="event.html").unwrap();assert!(e.open_artifact(a.path).unwrap_err().contains("QA"));
}
#[test]fn phase9_quality_rejects_missing_hidden_external_and_unsafe_material(){
 let good=html();inspect_html(&good,&plan(),INPUT).unwrap();
 for bad in [good.replace("当日受付","事前予約"),good.replace("<body>","<body onload='alert(1)'>"),good.replace("<body>","<body><script>alert(1)</script>"),good.replace("<body>","<body><img src='https://example.invalid/a.png'>"),good.replace("<body>","<body hidden>"),good.replace("<body>","<body><style>section{display:none}</style>"),good.replace("<body>","<body><meta content='0;url=https://x.invalid' http-equiv='refresh'>")]{assert!(inspect_html(&bad,&plan(),INPUT).is_err(),"unsafe/missing HTML accepted");}
 let hidden=good.replace("当日受付","<!--当日受付-->");assert!(inspect_html(&hidden,&plan(),INPUT).is_err());
 let head_only=good.replace("当日受付","").replace("<title>","<title>当日受付");assert!(inspect_html(&head_only,&plan(),INPUT).is_err());
 for style in ["background:u&#114;l(https://example.invalid/x)","background:u\\rl(https://example.invalid/x)"]{let bad=good.replace("<body>",&format!("<body style=\"{style}\">"));assert!(inspect_html(&bad,&plan(),INPUT).is_err());}
}
#[test]fn phase9_recovery_reuses_exact_artifact_and_rejects_duplicate_tamper_wrong_task(){
 let(e,p)=fixture();let n=e.snapshot().job.unwrap().workflow.nodes[0].clone();let raw=serde_json::to_string(&plan()).unwrap();e.accept_codex_output(&n,"plan.json",&raw).unwrap();e.accept_codex_output(&n,"plan.json",&raw).unwrap();assert_eq!(e.snapshot().job.unwrap().artifacts.len(),1);
 let s=e.snapshot();let mut j=s.job.unwrap();let target=&j.workflow.nodes[1];assert!(required_inputs(&j,target).is_err());j.nodes.get_mut("plan").unwrap().status="completed".into();assert!(required_inputs(&j,&j.workflow.nodes[1]).is_ok());
 j.artifacts.push(j.artifacts[0].clone());assert!(required_inputs(&j,&j.workflow.nodes[1]).is_err());j.artifacts.pop();j.artifacts[0].node_id="wrong-task".into();assert!(required_inputs(&j,&j.workflow.nodes[1]).is_err());
 j.artifacts[0].node_id="plan".into();j.artifacts[0].path=p.join("MISSING.json").to_string_lossy().into();assert!(required_inputs(&j,&j.workflow.nodes[1]).is_err());assert!(e.accept_codex_output(&n,"wrong-output.json",&raw).is_err());
 fs::write(p.join("plan.json"),"tampered").unwrap();assert!(e.accept_codex_output(&n,"plan.json",&raw).is_err());assert!(!s.events.iter().any(|x|x.event_type=="handoff_started"));
}
#[test]fn s1_css_and_aria_positive_regressions(){
 let good=html();
 for css in [".label{font-size:0.875rem}","p{font-size:0.5em}","p{font-size:1rem}","p{opacity:0.5}","@media print{body{margin:0;background:#fff}}","@media (max-width:600px){section{padding:1rem}}","@media screen and (max-width:600px){section{display:block}}","p{font-size:clamp(1rem,2vw,2rem)}"]{
  let v=good.replace("</style>",&format!("{css}</style>"));inspect_html(&v,&plan(),INPUT).unwrap_or_else(|e|panic!("{css}: {e}"));
 }
 for attr in ["aria-labelledby","aria-describedby"]{let v=good.replace("<section>",&format!("<section {attr}=\"label\"> ")).replacen("<h2>","<h2 id=\"label\">",1).replacen(&format!("<section {attr}=\"label\">"),"<section>",1);inspect_html(&v,&plan(),INPUT).unwrap();}
 let v=good.replace("<body>","<body aria-label=\"読書会の案内\">");inspect_html(&v,&plan(),INPUT).unwrap();
}
#[test]fn s1_parser_rejects_real_concealment_execution_network_bad_labels_and_missing_content(){
 let good=html();
 for css in ["font-size:0","font-size:0rem","font-size:0.0em","font-size:0%","font-size:clamp(0rem,1vw,1rem)","opacity:0","opacity:0.0","display:none","visibility:hidden","visibility:collapse","color:transparent","color:rgba(0,0,0,0)","color:#fff0","color:#ffffff00","width:0","height:0","margin-left:-9999px","position:absolute","clip-path:inset(100%)","content:'fake'","background:url(https://example.invalid/a)","background:u\\72l(https://example.invalid/a)","background:var(--x)","font-size:expression(alert(1))","font-size:calc(1rem - 1rem)","display:n\\6fne"]{
  let v=good.replace("</style>",&format!("p{{{css}}}</style>"));assert!(inspect_html(&v,&plan(),INPUT).is_err(),"accepted {css}");
 }
 for inserted in ["@import 'https://example.invalid/x';","@font-face{font-family:x;src:url(x)}","@media print{section{display:none}}","@media (max-width:600px){section{opacity:0}}","@supports(display:grid){p{color:red}}"]{assert!(inspect_html(&good.replace("</style>",&format!("{inserted}</style>")),&plan(),INPUT).is_err());}
 for inserted in ["<script>alert(1)</script>","<iframe src='javascript:alert(1)'></iframe>","<a href='javascript:alert(1)'>x</a>","<a href='https://example.invalid'>x</a>","<object data='data:text/html,x'></object>","<img src='//example.invalid/a'>","<meta http-equiv='refresh' content='0;url=https://example.invalid'>","<p onmouseover='alert(1)'>x</p>","<form action='https://example.invalid'></form>"]{assert!(inspect_html(&good.replace("<body>",&format!("<body>{inserted}")),&plan(),INPUT).is_err());}
 for v in [good.replace("<section>","<section aria-labelledby='missing'>"),good.replace("<section>","<section id='duplicate'>"),good.replace("<section>","<section aria-labelledby=''>"),good.replace("<body>","<body aria-hidden='true'>"),good.replace("<body>","<body hidden>"),good.replace("当日受付","<!--当日受付-->"),good.replace("当日受付",""),good.replace("<h1>","<h1 hidden>"),good.replace("</section>","</div>"),good.replace("<meta charset=\"utf-8\">","")]{assert!(inspect_html(&v,&plan(),INPUT).is_err());}
}
#[test]fn s1_exact_original_passes_general_parser_without_whitelist_exception(){
 let r=PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
 let raw=fs::read_to_string(r.join("tests/fixtures/rejected-output/turn2-original.html")).unwrap();
 let p:Plan=serde_json::from_slice(&fs::read(r.join("tests/fixtures/rejected-output/turn1-original.json")).unwrap()).unwrap();
 inspect_html(&raw,&p,INPUT).unwrap();
 assert!(inspect_html(&raw.replace("font-size: 0.875rem","font-size: 0rem"),&p,INPUT).is_err());
}
#[test]fn phase9_image_full_decode_is_file_integrity_not_original_provenance(){
 let source=PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("public/sprites/scholar.png");let bytes=fs::read(source).unwrap();assert_eq!(inspect_image_bytes(&bytes).unwrap(),(128,288));assert!(inspect_image_bytes(b"\x89PNG\r\n\x1a\ninvalid").is_err());assert!(inspect_image_bytes(&bytes[..bytes.len()/2]).is_err());assert!(inspect_image_bytes(b"a screenshot filename is not provenance").is_err());
}
#[test]fn phase9_offline_whole_product_persistence_and_hash_not_live_e2e(){
 let(e,p)=fixture();let ns=e.snapshot().job.unwrap().workflow.nodes;let plan_raw=serde_json::to_string(&plan()).unwrap();e.accept_codex_output(&ns[0],"plan.json",&plan_raw).unwrap();e.update(|s|{s.job.as_mut().unwrap().nodes.get_mut("plan").unwrap().status="running".into();Ok(())}).unwrap();e.update(|s|{s.job.as_mut().unwrap().nodes.get_mut("plan").unwrap().status="completed".into();Ok(())}).unwrap();
 e.accept_codex_output(&ns[1],"event.html",&html()).unwrap();let s=e.snapshot();let j=s.job.as_ref().unwrap();let q=verify_product(j).unwrap();assert_eq!(q.display_status,"NOT_VERIFIED");assert_eq!(j.practical.as_ref().unwrap().turn_starts,0);
 fs::write(p.join("fixture-quality.json"),serde_json::to_vec_pretty(&q).unwrap()).unwrap();let before=j.artifacts[1].sha256.clone();drop(e);let restored=Engine::open(&p,None).unwrap();let j=restored.snapshot().job.unwrap();assert_eq!(verify_product(&j).unwrap().artifact_sha256,before);assert_eq!(j.status,"disconnected");assert!(restored.control("retry".into()).is_err());
 fs::write(p.join("event.html"),"changed").unwrap();assert!(verify_product(&j).is_err());
 fs::write(p.join("offline-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"classification":"OFFLINE_VERIFIED","realE2e":"NOT_RUN","inference":0,"fixtureHtmlHash":before,"display":"NOT_VERIFIED"})).unwrap()).unwrap();
}
