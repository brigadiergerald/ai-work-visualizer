//! Bounded document jobs. Reuses the original Engine, store and formal recovery.
//! No inference, transport, browser or alternate model is started here.
use crate::{model::*,practical::{Plan,Quality}};
use sha2::{Digest,Sha256};

pub fn legacy(input:&str)->bool{input.contains("森の図書室")||(input.contains("開催案内")&&input.contains("運営チェックリスト"))}
pub fn format(input:&str)->&'static str{
 let s=input.to_ascii_lowercase();
 if s.contains("html"){"html"}else if s.contains("json"){"json"}else if s.contains("txt")||s.contains("プレーンテキスト"){"txt"}else{"md"}
}
pub fn filename(input:&str)->String{format!("document.{}",format(input))}
pub fn exact_copy(input:&str)->Vec<String>{
 let re=regex::Regex::new("「([^「」\\n]+)」").unwrap();
 re.captures_iter(input).map(|c|c[1].to_string()).collect()
}
pub fn workflow(input:&str)->Workflow{
 let mut w=crate::practical::workflow();w.workflow_id="codex-document-v1".into();w.workflow_name="文書の実務制作（Codex）".into();
 let name=filename(input);
 w.nodes[0].work_instruction="ユーザー原文から納品本文の内容と内部の制作条件を区分する。JSONのみで {\"title\":文字列,\"requirements\":[読者に届ける必須の本文文言],\"checklist\":[本文に載せる具体的な項目、不要なら空配列],\"productionConstraints\":[目的・対象者・出力形式・構成・禁止事項など、制作時に守る条件]}。requirementsは1件以上。原文で指定された表示文言は原文どおりrequirementsへ含める。形式・項目数などの条件を説明する文章は、表示が明示指定された場合を除きproductionConstraintsへ入れ、本文必須文言にはしない。事実・素材の不足はUNKNOWNとし創作しない。外部調査、公開、課金、画像制作は実行しない。重大な不足で作れない場合はJSONを創作せず不足を報告する（ホストが停止する）。".into();
 let condition=match format(input){
  "html"=>"DOCTYPEから閉じhtmlまでの日本語静的HTML。lang=ja、UTF-8、viewport、title、h1、sectionを含める。明るく読みやすく、CSSはstyle内。外部URL、画像、script、フォーム、埋込、イベント属性は禁止。",
  "json"=>"有効なJSONだけ。Markdown囲みや説明は禁止。",
  "txt"=>"プレーンテキスト本文だけ。HTMLやMarkdown囲みは禁止。",
  _=>"日本語Markdown本文だけ。HTML、外部画像、コードブロックは禁止。"
 };
 w.nodes[1].work_instruction=format!("ユーザーがそのまま使える完成文書だけを作成する。{condition} 原文の目的・対象者・制約とplan.jsonのproductionConstraintsに従い、requirements/checklistの全項目を本文に含める。内部条件やAIの制作計画・作成報告を納品本文へ転記しない。原文で表示を明示された文言は保持する。読者が使い方や判断条件を理解するための正当な導入文は含めてよい。不明な実体験・数値・引用・外部事実を創作しない。");
 w.nodes[1].label="文書制作".into();w.nodes[1].output_artifacts=vec![name.clone()];
 w.nodes[2].input_artifacts=vec![name.clone(),"plan.json".into()];
 w.nodes[2].work_instruction="確定要件・原文の指定文言・形式・安全性・実file/hashを検査。意味内容と表示の実務QAは別判定。".into();
 w.nodes[3].input_artifacts=vec![name,"quality.json".into()];w
}
/// New jobs only. Historical documents keep the original inspection and hashes.
pub fn inspect_versioned(raw:&str,plan:&Plan,input:&str,version:u32)->Result<Quality,String>{
 let mut quality=inspect(raw,plan,input)?;
 if version<4{return Ok(quality)}
 let locks=exact_copy(input);
 // Reject explicit production commentary; never strip or rewrite a final response.
 // Introductions about the reader's use are allowed. This is intentionally narrow;
 // semantic usefulness still needs an independent content review.
 let report=regex::Regex::new(r"^(?:[-*]\s*)?(?:ご依頼(?:どおり|のとおり)|以下の条件で作成|(?:Markdown|HTML|JSON|プレーンテキスト)形式で作成|AIとして|制作条件[：:]|制作計画[：:]|構成・形式[：:]|最終出力の受入条件[：:])").unwrap();
 for line in raw.lines(){let line=line.trim().trim_start_matches('#').trim();
  if report.is_match(line)&&!locks.iter().any(|lock|line.contains(lock)){return Err("納品本文に内部の制作条件・作成報告が残っています。本文は改変せず停止しました。".into())}
 }
 for internal in &plan.production_constraints{
  if !internal.trim().is_empty()&&raw.lines().any(|line|line.trim()==internal.trim())&&!locks.iter().any(|lock|lock==internal)&&!plan.requirements.contains(internal)&&!plan.checklist.contains(internal){return Err("本文用でない制作条件がそのまま転記されています。".into())}
 }
 quality.checks.push("v4 delivery content separated from production constraints; legitimate introductions preserved; semantic QA separate".into());
 Ok(quality)
}
pub fn inspect(raw:&str,plan:&Plan,input:&str)->Result<Quality,String>{
 if raw.trim().is_empty()||raw.len()>2*1024*1024||raw.chars().any(|c|c.is_control()&&!['\n','\r','\t'].contains(&c)){return Err("文書が空・サイズ超過・不正制御文字です".into())}
 let text=match format(input){
  "html"=>{let d=crate::html_policy::inspect(raw)?;crate::html_policy::require_document_structure(&d)?;d.text},
  "json"=>{let v:serde_json::Value=serde_json::from_str(raw).map_err(|e|format!("完成JSONが不正です: {e}"))?;fn strings(v:&serde_json::Value,out:&mut Vec<String>){match v{serde_json::Value::String(s)=>out.push(s.clone()),serde_json::Value::Array(a)=>for x in a{strings(x,out)},serde_json::Value::Object(o)=>for x in o.values(){strings(x,out)},_=>{}}}let mut out=vec![];strings(&v,&mut out);out.join("\n")},
  _=>{if raw.contains("```")||regex::Regex::new(r"(?i)<\s*[!/?a-z]|!\[[^\]]*\]\(").unwrap().is_match(raw){return Err("完成文書に許可外のHTML・画像・コード囲みがあります".into())}raw.into()}
 };
 for lock in plan.requirements.iter().chain(&plan.checklist).chain(exact_copy(input).iter()){
  if !text.contains(lock){return Err(format!("完成文書に必須文言がありません: {lock}"))}
 }
 if format(input)=="md"&&input.contains("チェックボックス")&&!regex::Regex::new(r"(?m)^\s*[-*+] \[ \] \S.*$").unwrap().is_match(raw){
  return Err("指定されたMarkdownの未チェックのチェックボックスがありません".into())
 }
 Ok(Quality{artifact_sha256:crate::hashing::hex(&Sha256::digest(raw.as_bytes())),machine_status:"PASS".into(),display_status:"NOT_VERIFIED".into(),checked_at:now(),checks:vec!["document format and restricted content".into(),"all planned requirements and independent quoted original copy".into(),"semantic/user-value review and display remain separate".into()]})
}
