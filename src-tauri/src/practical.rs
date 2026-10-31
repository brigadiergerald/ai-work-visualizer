//! Host-owned practical HTML pipeline, using the existing Job/node/RPC/SQLite owner.
//! This module never starts a model or a browser itself.
use crate::model::*;
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{fs,path::Path};

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Plan { pub title:String, pub requirements:Vec<String>, pub checklist:Vec<String>, #[serde(default,skip_serializing_if="Vec::is_empty")] pub production_constraints:Vec<String> }
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Practical { pub version:u32, pub original_instruction:String, pub turn_starts:u32, pub max_turn_starts:u32, pub plan:Option<Plan> }
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Quality { pub artifact_sha256:String,pub machine_status:String,pub display_status:String,pub checked_at:String,pub checks:Vec<String> }

pub fn intake(instruction:&str)->Result<Option<Practical>,String>{
 let lower=instruction.to_lowercase();
 if !lower.contains("html")&&!regex::Regex::new(r"[ぁ-んァ-ヶ一-龠]").unwrap().is_match(instruction){return Ok(None)}
 let mut requested=lower.clone();
 // Explicit absence/prohibition is not a request for an unavailable material.
 // Unrecognised or contradictory wording remains conservatively blocked.
 // A checklist can discuss checking existing images without requesting an image
 // asset. Only this review topic, paired with an explicit generation exclusion,
 // is exempt; any remaining request for images still fails closed below.
 if ["画像生成は不要","画像生成不要","画像生成なし","画像生成無し","画像を生成しない"].iter().any(|x|lower.contains(x)){
  requested=requested.replace("文章や画像の内容確認","");
 }
 for subject in ["画像生成","画像","挿絵","イラスト","chatgpt","外部調査","ウェブ検索","公開","投稿"]{
  for suffix in ["なし","無し","不要","は不要","を使わない","は使わない","を生成しない","はしない","しない"]{requested=requested.replace(&format!("{subject}{suffix}"),"");}
 }
 // Preserve original S1 contracts; new jobs use the bounded document pipeline.
 if instruction.trim().is_empty()||instruction.len()>64000{return Err("指示を1〜64,000バイトで入力してください".into())}
 if instruction.contains("森の図書室"){for lock in ["2026年11月14日","14:00","16:00","当日受付","架空"]{if !instruction.contains(lock){return Err(format!("S1入力の固定条件が不足しています: {lock}"))}}}
 if ["画像","挿絵","イラスト","chatgpt"].iter().any(|s|requested.contains(s)){
  return Err("このHTMLは通常ChatGPT素材を必要とします。正式Edge Taskの実行host接続が未検証のため、推論・手動画像登録で代替せず停止します。".into())
 }
 if !crate::codex_only::legacy(instruction)&&["pdf","pptx","xlsx","動画","音声","アプリを","実装して","外部調査","ウェブ検索","投稿して","公開して"].iter().any(|s|requested.contains(s)){return Err("現在の自動完成範囲はローカル文書（HTML / Markdown / TXT / JSON）です。必要な素材・実行方式を確認してから進めてください。".into())}
 Ok(Some(Practical{version:if crate::codex_only::legacy(instruction){1}else{4},original_instruction:instruction.into(),turn_starts:0,max_turn_starts:2,plan:None}))
}
pub fn workflow_for(p:&Practical)->Workflow{if p.version==5{crate::general::workflow()}else if p.version>=3{crate::codex_only::workflow(&p.original_instruction)}else{workflow()}}
pub fn workflow()->Workflow{
 let defs=[("plan","要件・工程決定","scholar","codex",vec![],vec![],"plan.json","ユーザー原文から開催案内と運営チェックリストの必須条件を抽出。出力はJSONだけ。schema: {\"title\":文字列,\"requirements\":[最終HTMLに表示する必須条件の原文文字列],\"checklist\":[具体的な運営作業を3件以上]}。日時・会場・参加方法・架空の試験であることを保持し、不明な情報を創作しない。"),
 ("writing","HTML制作","scribe","codex",vec!["plan"],vec!["plan.json"],"event.html","日本語の開催案内と運営チェックリストを、明るく読みやすい1つの完成HTMLにする。DOCTYPEから閉じhtmlまで本文のみ。Markdown囲みや付随説明なし。lang=ja、meta charset=utf-8、title、h1、案内と運営チェックリストの各section、3件以上のli、viewportを含める。要件JSONのrequirements/checklistをすべてそのまま可視テキストに含める。外部通信・リンク・画像・script・フォーム・iframe・埋込・イベント属性を使わずCSSはstyle内へ。"),
 ("qa","完成品QA","analyst","quality",vec!["writing"],vec!["event.html","plan.json"],"quality.json","原文・要件・HTMLの可視内容、自己完結性、保存範囲と実hashをホストが検査。表示確認は別証拠とする。"),
 ("delivery","完成品保存","messenger","save",vec!["qa"],vec!["event.html","quality.json"],"manifest.json","完成HTMLとhashを再照合して保存。manifestだけを完成品にしない。")];
 let nodes=defs.into_iter().map(|(id,label,sprite,executor,deps,inputs,output,work)|Node{node_id:id.into(),role:if executor=="codex"{"Codex".into()}else{"Codexホスト".into()},label:label.into(),sprite_key:sprite.into(),executor:executor.into(),dependencies:deps.into_iter().map(String::from).collect(),input_artifacts:inputs.into_iter().map(String::from).collect(),output_artifacts:vec![output.into()],work_instruction:work.into()}).collect::<Vec<_>>();
 let edges=nodes.iter().flat_map(|n|n.dependencies.iter().map(|d|Edge{edge_id:format!("{d}-{}",n.node_id),from_node_id:d.clone(),to_node_id:n.node_id.clone()})).collect();
 Workflow{workflow_id:"practical-html-v1".into(),workflow_name:"HTMLの実務制作（Codex）".into(),nodes,edges}
}
pub fn parse_plan(raw:&str,instruction:&str)->Result<Plan,String>{
 let p:Plan=serde_json::from_str(raw).map_err(|e|format!("要件JSONが不正です。再推論は自動実行しません: {e}"))?;
 if p.title.trim().is_empty()||p.requirements.is_empty()||(crate::codex_only::legacy(instruction)&&p.checklist.len()<3)||p.requirements.iter().chain(&p.checklist).chain(&p.production_constraints).any(|x|x.trim().is_empty()||x.len()>2000){return Err("必要な要件・本文項目がありません（開催案内は3件以上の運営作業が必要です）".into())}
 if !crate::codex_only::legacy(instruction){for lock in crate::codex_only::exact_copy(instruction){if p.title!=lock&&!p.requirements.iter().any(|x|x.contains(&lock)){return Err(format!("原文の指定文言が要件から欠落: {lock}"))}}}
 // The S1 locks are independent of model-authored acceptance conditions.
 if instruction.contains("森の図書室"){
  let full=p.requirements.join("\n");
  for lock in ["2026年11月14日","14:00","16:00","森の図書室","当日受付"]{if !instruction.contains(lock)||!full.contains(lock){return Err(format!("S1原文と抽出要件の固定条件が不足: {lock}"))}}
  if !full.contains("架空"){return Err("架空の試験であることが要件から欠落".into())}
 }
 Ok(p)
}
pub fn inspect_html(raw:&str,plan:&Plan,instruction:&str)->Result<Quality,String>{
 // Safety/syntax/ID graph first, then independent product structure/content.
 let doc=crate::html_policy::inspect(raw)?;
 inspect_document(raw,&doc,plan,instruction)
}
pub(crate) fn inspect_document(raw:&str,doc:&crate::html_policy::Document,plan:&Plan,instruction:&str)->Result<Quality,String>{
 crate::html_policy::require_structure(&doc)?;
 let text=&doc.text;
 for requirement in plan.requirements.iter().chain(&plan.checklist){if !text.contains(requirement){return Err(format!("表示本文に必須条件/運営作業がありません: {requirement}"))}}
 if instruction.contains("森の図書室"){for lock in ["2026年11月14日","14:00","16:00","森の図書室","当日受付","架空"]{if !text.contains(lock){return Err(format!("完成品にS1固定条件がありません: {lock}"))}}}
 if !text.contains("開催案内")||!text.contains("運営チェックリスト"){return Err("開催案内と運営チェックリストの2区分が必要です".into())}
 Ok(Quality{artifact_sha256:format!("{:x}",Sha256::digest(raw.as_bytes())),machine_status:"PASS".into(),display_status:"NOT_VERIFIED".into(),checked_at:now(),checks:vec!["original instruction + independent S1 locks".into(),"all planned requirements/checklist in tokenized body text; visual readability separate".into(),"restricted static HTML tags/attributes/CSS".into(),"Japanese sections and checklist structure".into()]})
}
pub fn required_inputs(j:&Job,n:&Node)->Result<Vec<Artifact>,String>{
 let mut files=vec![];
 let mut ancestors=std::collections::HashSet::new();let mut pending=n.dependencies.clone();
 while let Some(id)=pending.pop(){if ancestors.insert(id.clone()){let source=j.workflow.nodes.iter().find(|x|x.node_id==id).ok_or("入力元工程がありません")?;pending.extend(source.dependencies.clone());}}
 // Every named input must resolve exactly once, including an earlier ancestor.
 for name in &n.input_artifacts{
  let matches=j.artifacts.iter().filter(|a|&a.name==name).collect::<Vec<_>>();
  if matches.len()!=1{return Err(format!("必須入力が欠落・重複: {name}"))}
  if !ancestors.contains(&matches[0].node_id){return Err(format!("別Taskの入力です: {name}"))}
  if j.nodes.get(&matches[0].node_id).map(|r|r.status.as_str())!=Some("completed"){return Err(format!("未完了工程の入力: {name}"))}
  files.push(matches[0].clone());
 }
 for dependency in &n.dependencies{
  let sources=j.artifacts.iter().filter(|a|&a.node_id==dependency).collect::<Vec<_>>();
  if sources.is_empty(){return Err(format!("前工程の実成果物がありません: {dependency}"))}
  for a in sources{if !files.iter().any(|f|f.path==a.path){files.push(a.clone())}}
 }
 for a in &files{crate::engine::verify_artifact(a,Path::new(&j.workspace))?;}
 Ok(files)
}
pub fn verify_product(j:&Job)->Result<Quality,String>{
 if crate::general::applies(j){return crate::general::verify_product(j)}
 if j.collaboration.is_some(){return crate::s2_core::verify_product(j)}
 for a in &j.artifacts{crate::engine::verify_artifact(a,Path::new(&j.workspace))?;}
 let practical=j.practical.as_ref().ok_or("実務契約がありません")?;
 let plan=practical.plan.as_ref().ok_or("工程決定が未完了です")?;
 let name=if j.workflow_id=="codex-document-v1"{crate::codex_only::filename(&practical.original_instruction)}else{"event.html".into()};
 let files=j.artifacts.iter().filter(|a|a.name==name&&a.node_id=="writing").collect::<Vec<_>>();
 if files.len()!=1{return Err("完成HTMLが欠落・重複しています".into())}
 crate::engine::verify_artifact(files[0],Path::new(&j.workspace))?;
 let raw=fs::read_to_string(&files[0].path).map_err(|e|e.to_string())?;
 let q=if j.workflow_id=="codex-document-v1"{crate::codex_only::inspect_versioned(&raw,plan,&practical.original_instruction,practical.version)?}else{inspect_html(&raw,plan,&practical.original_instruction)?};
 if q.artifact_sha256!=files[0].sha256{return Err("完成品QAと登録hashが一致しません".into())}
 Ok(q)
}
