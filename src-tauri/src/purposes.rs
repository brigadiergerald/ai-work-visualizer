//! Additive Purpose contract v1. Only trusted Core factories select a workflow.
use crate::model::{Job,Snapshot,Workflow};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};

pub const SOCIAL:&str="social-text";
pub const VERSION:&str="1.3.0";
pub const FOUR_ROLE_VERSION:&str="1.2.0";
pub const PRIOR_VERSION:&str="1.1.0";
pub const LEGACY_VERSION:&str="1.0.0";
pub const ROLE_MAP_VERSION:&str="social-text-roles-v1";
pub const MAX_BYTES:usize=64_000;
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct SocialTextInput{pub theme:String,pub content_language:String,#[serde(default,skip_serializing_if="Option::is_none")]pub context:Option<String>}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct JobContext{
 pub version:u32,pub job_id:String,pub purpose_id:String,pub definition_version:String,pub role_map_version:String,
 pub content_language:String,pub submitted_input:Value,pub input_sha256:String,pub workflow_sha256:String,
}
pub struct Prepared{pub instruction:String,pub workflow:Workflow,pub context:JobContext}
pub fn sha256(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
pub fn catalog()->Value{json!({"contractVersion":1,"purposes":[
 {"id":SOCIAL,"definitionVersion":VERSION,"roleMapVersion":crate::sns_team::ROLE_MAP,"label":"SNS文章制作","description":"調査・ディレクター・製作・検品と投稿。今回は接続先の確認まで","availability":"READY","reasonCode":null,"publicationMode":"PREPARATION_ONLY",
 "inputSchema":{"type":"object","additionalProperties":false,"required":["theme","contentLanguage"],"maxInstructionUtf8Bytes":MAX_BYTES,"properties":{"theme":{"type":"string","minLength":1,"maxUtf8Bytes":MAX_BYTES},"contentLanguage":{"type":"string","enum":["ja","en"]},"context":{"type":"string","maxUtf8Bytes":MAX_BYTES}}}},
 {"id":"research","definitionVersion":VERSION,"roleMapVersion":null,"label":"情報収集","description":"資料収集・出典確認のWorkflowは準備中","availability":"PLANNED","reasonCode":"WORKFLOW_NOT_IMPLEMENTED","publicationMode":"PREPARATION_ONLY",
 "inputSchema":{"type":"object","additionalProperties":false,"required":["objective","contentLanguage","sourceRefs","acquisitionMode"],"properties":{"objective":{"type":"string","maxUtf8Bytes":MAX_BYTES},"contentLanguage":{"type":"string","enum":["ja","en"]},"sourceRefs":{"type":"array","items":{"type":"string"}},"acquisitionMode":{"type":"string","enum":["PROVIDED_SOURCES","APPROVED_FETCH"]}}}}
 ]})}
pub fn prepare(purpose:&str,version:&str,input:Value)->Result<Prepared,String>{
 if ![SOCIAL,"research"].contains(&purpose){return Err("未知のpurposeIdです".into())}
 if ![VERSION,FOUR_ROLE_VERSION,PRIOR_VERSION,LEGACY_VERSION].contains(&version){return Err("Purpose定義versionが一致しません。用途一覧を再取得してください".into())}
 if purpose!="social-text"{return Err("情報収集は準備中です。開始できません".into())}
 if input.get("context").is_some_and(Value::is_null){return Err("contextは省略するか文字列で入力してください".into())}
 let v:SocialTextInput=serde_json::from_value(input.clone()).map_err(|e|format!("SNS用途の入力が不正です: {e}"))?;
 if v.theme.trim().is_empty()||v.theme.len()>MAX_BYTES||v.context.as_ref().is_some_and(|c|c.len()>MAX_BYTES){return Err("投稿テーマは必須です。入力は64,000 UTF-8 bytes以内にしてください".into())}
 let language=match v.content_language.as_str(){"ja"=>"日本語 (ja)","en"=>"英語 (en)",_=>return Err("contentLanguageはjaまたはenです".into())};
 // Wrappers are Core-owned; the original strings (including CRLF/whitespace) are untouched.
 let instruction=format!("生成する本文の言語: {language}\nUI表示言語とは独立した指定です。原文の固定文言・引用は維持してください。\n\n投稿テーマ:\n{}{}",v.theme,v.context.as_ref().map(|c|format!("\n\n補足情報（原文）:\n{c}")).unwrap_or_default());
 if instruction.len()>MAX_BYTES{return Err("言語指定を含む開始指示が64,000 UTF-8 bytesを超えています".into())}
 let mut workflow=if version==LEGACY_VERSION{crate::sns::workflow()}else if version==PRIOR_VERSION{crate::sns::autonomous_workflow()}else if version==FOUR_ROLE_VERSION{crate::sns_team::workflow()}else{crate::sns_team::decision_workflow()};
 for n in &mut workflow.nodes{if ["research","plan","writing"].contains(&n.node_id.as_str()){n.work_instruction.push_str(&format!("\n本文の生成言語は{language}。UI表示言語から推測しない。原文の固定文言・引用は維持する。"));}}
 let context=JobContext{version:1,job_id:String::new(),purpose_id:purpose.into(),definition_version:version.into(),role_map_version:if [VERSION,FOUR_ROLE_VERSION].contains(&version){crate::sns_team::ROLE_MAP}else{ROLE_MAP_VERSION}.into(),content_language:v.content_language,submitted_input:input.clone(),input_sha256:sha256(&serde_json::to_vec(&input).map_err(|e|e.to_string())?),workflow_sha256:sha256(&serde_json::to_vec(&workflow).map_err(|e|e.to_string())?)};
 Ok(Prepared{instruction,workflow,context})
}
pub fn validate_context(context:&JobContext,job:&Job)->Result<(),String>{
 let prepared=prepare(&context.purpose_id,&context.definition_version,context.submitted_input.clone())?;
 let mut expected=prepared.context;expected.job_id=job.job_id.clone();
 if context!=&expected||context.job_id!=job.job_id||prepared.instruction!=job.instruction||context.workflow_sha256!=sha256(&serde_json::to_vec(&job.workflow).map_err(|e|e.to_string())?){return Err("JobとPurpose contextの所属・入力・Workflow hashが一致しません".into())}Ok(())
}
pub fn view(j:&Job,s:&Snapshot)->Value{
 if let Some(c)=s.purpose_job_context.get(&j.job_id){json!({"jobId":j.job_id,"purposeId":c.purpose_id,"definitionVersion":c.definition_version,"roleMapVersion":c.role_map_version,"contentLanguage":c.content_language,"inputSha256":c.input_sha256,"contextSource":"PERSISTED_NEW_JOB"})}
 else{json!({"jobId":j.job_id,"purposeId":if crate::sns::applies(j){Some(SOCIAL)}else{None},"definitionVersion":null,"roleMapVersion":null,"contentLanguage":null,"inputSha256":null,"contextSource":if crate::sns::applies(j){"LEGACY_SNS_PROJECTION"}else{"UNCLASSIFIED"}})}
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn purpose_catalog_ready_and_planned(){let c=catalog();assert_eq!(c["purposes"][0]["availability"],"READY");assert_eq!(c["purposes"][1]["availability"],"PLANNED");assert_eq!(c["purposes"][0]["inputSchema"]["additionalProperties"],false);}
 #[test]fn purpose_validator_rejects_unknown_planned_versions_and_fields(){for (id,ver,v) in [("other",VERSION,json!({})),("research",VERSION,json!({})),(SOCIAL,"0",json!({})),(SOCIAL,VERSION,json!({})),(SOCIAL,VERSION,json!({"theme":" ","contentLanguage":"ja"})),(SOCIAL,VERSION,json!({"theme":"a","contentLanguage":"fr"})), (SOCIAL,VERSION,json!({"theme":"a","contentLanguage":"ja","context":null})),(SOCIAL,VERSION,json!({"theme":"a","contentLanguage":"ja","executor":"codex"})),(SOCIAL,VERSION,json!({"theme":"a","contentLanguage":"en","workflow":{}}))]{assert!(prepare(id,ver,v).is_err());}}
 #[test]fn purpose_builder_preserves_exact_copy_and_languages(){for lang in ["ja","en"]{let v=json!({"theme":"  固定文言\r\n次の行  ","context":"対象\n\n文脈\t末尾 ","contentLanguage":lang});let p=prepare(SOCIAL,VERSION,v.clone()).unwrap();assert!(p.instruction.contains("  固定文言\r\n次の行  "));assert!(p.instruction.contains("対象\n\n文脈\t末尾 "));assert_eq!(p.context.submitted_input,v);assert_eq!(p.context.content_language,lang);assert_eq!(p.workflow.workflow_id,crate::sns_team::DECISIONS_ID);assert!(p.workflow.nodes.iter().any(|n|n.node_id=="research"));assert!(!p.workflow.nodes.iter().any(|n|n.node_id==crate::sns::REVIEW));assert!(p.workflow.nodes[0].work_instruction.contains(if lang=="en"{"英語 (en)"}else{"日本語 (ja)"}));}}
 #[test]fn purpose_utf8_and_built_instruction_byte_limit(){assert!(prepare(SOCIAL,VERSION,json!({"theme":"界".repeat(21_334),"contentLanguage":"ja"})).is_err());assert!(prepare(SOCIAL,VERSION,json!({"theme":"x".repeat(64_000),"contentLanguage":"en"})).is_err());assert!(prepare(SOCIAL,VERSION,json!({"theme":"界".repeat(10_000),"context":"y".repeat(34_000),"contentLanguage":"en"})).is_err());assert!(prepare(SOCIAL,VERSION,json!({"theme":"界".repeat(20_000),"contentLanguage":"ja"})).is_ok());let overhead=prepare(SOCIAL,VERSION,json!({"theme":"a","contentLanguage":"en"})).unwrap().instruction.len()-1;assert_eq!(prepare(SOCIAL,VERSION,json!({"theme":"a".repeat(MAX_BYTES-overhead),"contentLanguage":"en"})).unwrap().instruction.len(),MAX_BYTES);assert!(prepare(SOCIAL,VERSION,json!({"theme":"a".repeat(MAX_BYTES-overhead+1),"contentLanguage":"en"})).is_err());}
}
