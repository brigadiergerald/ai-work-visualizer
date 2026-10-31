//! New SNS jobs: researcher -> director -> creator -> inspection/delivery.
//! Historical workflows and their human approval records remain unchanged.
use crate::model::*;
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
pub const ID:&str="sns-four-person-v1";
pub const DECISIONS_ID:&str="sns-decisions-v1";
pub const ROLE_MAP:&str="social-text-roles-v2";
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Research {pub topic:String,pub evidence_scope:String,pub provided_facts:Vec<String>,pub candidate_angles:Vec<String>,pub unknowns:Vec<String>,pub limitations:Vec<String>}
pub fn validate_research(raw:&str)->Result<(),String>{
 let r:Research=serde_json::from_str(raw).map_err(|e|format!("調査結果の形式が不正です: {e}"))?;
 if r.evidence_scope!="PROVIDED_INPUT_ONLY"||r.topic.trim().is_empty()||r.candidate_angles.is_empty()||r.limitations.is_empty()||r.provided_facts.iter().chain(&r.candidate_angles).chain(&r.unknowns).chain(&r.limitations).any(|s|s.trim().is_empty()||s.len()>8000){return Err("調査結果の題材・候補・不確実性・実証範囲が不足しています".into())}Ok(())
}
pub fn research_schema()->Value{
 let strings=json!({"type":"array","items":{"type":"string"}});
 json!({"type":"object","additionalProperties":false,"properties":{"topic":{"type":"string"},"evidenceScope":{"type":"string","enum":["PROVIDED_INPUT_ONLY"]},"providedFacts":strings,"candidateAngles":strings,"unknowns":strings,"limitations":strings},"required":["topic","evidenceScope","providedFacts","candidateAngles","unknowns","limitations"]})
}
pub fn workflow()->Workflow{
 let mut w=crate::sns::autonomous_workflow();w.workflow_id=ID.into();w.workflow_name="SNS文章制作：調査・ディレクター・製作・検品と投稿".into();
 w.nodes.retain(|n|n.node_id!=crate::sns::REVIEW);
 for n in &mut w.nodes{match n.node_id.as_str(){
  "plan"=>{n.label="ディレクター：企画・要件整理".into();n.dependencies=vec!["research".into()];n.input_artifacts.push("research.json".into());n.work_instruction.push_str("\nディレクターとしてresearch.jsonを読み、候補から今回の読者に価値がある切り口を選ぶ。中心メッセージ、構成、長さ、文体、正確な文言、受入条件を具体化する。調査の不確実性・外部照合未実施を保持し、確認済み事実へ昇格させない。人間の内容承認を待たず、検査後にローカル納品する計画にする。");},
  "writing"=>{n.label="製作：文章作成".into();n.input_artifacts.push("research.json".into());n.work_instruction.push_str("\n制作担当としてディレクターのplan.jsonと調査結果を使用する。調査の限界・未確認事項を守り、根拠のない体験・出典・数値を作らない。機械検査・ローカル保存は後続の内部処理が行い、人間の内容承認待ちは設けない。");},
  "qa"=>{n.label="検品・投稿：品質検査".into();},
  "delivery"=>{n.label="検品・投稿：出力・保存・接続先確認".into();n.dependencies=vec!["qa".into()];n.input_artifacts.retain(|s|s!="review.json");n.output_artifacts.push("connector-check.json".into());},_=>{}
 }}
 w.nodes.insert(0,Node{node_id:"research".into(),role:"Codex".into(),label:"調査：素材・事実・切り口の整理".into(),sprite_key:"scholar".into(),executor:"codex".into(),dependencies:vec![],input_artifacts:vec![],output_artifacts:vec!["research.json".into()],work_instruction:format!("調査担当として、ユーザー原文と提供された素材を読み、テーマに使える事実、切り口候補、不明点を整理する。{}\n現在の実証範囲は提供情報と素材だけ。外部検索・外部資料照合は行わず、調査済みと称しない。提供情報にない一般知識は確認済み事実に混ぜずunknownsへ入れ、根拠が不足する主張は制作候補から外す。ディレクターが選べる具体的な候補をcandidateAnglesへ返す。evidenceScopeはPROVIDED_INPUT_ONLY。JSONだけ: {{\"topic\":テーマ,\"evidenceScope\":\"PROVIDED_INPUT_ONLY\",\"providedFacts\":[提供された事実],\"candidateAngles\":[切り口候補],\"unknowns\":[不明点],\"limitations\":[確認できた範囲と未実施の調査]}}。外部公開・投稿・予約・購入・削除は行わない。",crate::sns::AUTONOMOUS_POLICY)});
 w.edges=w.nodes.iter().flat_map(|n|n.dependencies.iter().map(|d|Edge{edge_id:format!("{d}-{}",n.node_id),from_node_id:d.clone(),to_node_id:n.node_id.clone()})).collect();w
}
pub fn applies(j:&Job)->bool{[ID,DECISIONS_ID].contains(&j.workflow_id.as_str())}
/// Versioned factory: never change the original workflow bytes or old Purpose hashes.
pub fn decision_workflow()->Workflow{
 let mut w=workflow();w.workflow_id=DECISIONS_ID.into();
 for n in &mut w.nodes{match n.node_id.as_str(){
  "plan"=>n.work_instruction.push_str("\n企画が確定するこの工程で、利用者向けの短い判断記録decisionNotesをJSONへ追加する。themeは実際に選んだテーマ・重視した指示・採用理由、directionは対象読者・構成/文体/訴求・目的への適合理由を記す。各項目はsummary（選択概要1文）、reason（理由1文）、details（具体的な判断と限界の短い配列）。内部の思考過程ではなく確定した方針の要約だけ。原文や素材にない事実を追加しない。"),
  "writing"=>n.work_instruction.push_str("\n文章を確定するこの工程で、利用者向けの短い判断記録decisionNotes.writingをJSONへ追加する。summary（冒頭・本文・結びの構成1文）、reason（採用表現の理由1文）、details（何を強調/省略したか、具体的表現と限界の短い配列）。完成後の別工程で理由を推測させず、この文章と同じ回答で記録する。内部の思考過程は不要。判断記録をpost.txtへ混入させない。"),_=>{}
 }}w
}
/// Current user scope is boundary checking only. Never calls Check/SendMock or a write transport.
pub fn check_connectors(e:&std::sync::Arc<crate::engine::Engine>,n:&Node)->Result<(),String>{
 let s=e.snapshot();let j=s.job.as_ref().ok_or("Jobがありません")?;
 if !applies(j)||n.node_id!="delivery"{return Err("コネクタ確認の工程が一致しません".into())}
 crate::general::verify_product(j)?;
 let post=j.artifacts.iter().find(|a|a.name=="post.txt"&&a.kind=="product").ok_or("検品対象の投稿文がありません")?;
 let inventory=crate::connectors::perform(e,crate::connectors::Action::Get)?;
 let mut checks=vec![];
 for c in inventory["connectors"].as_array().ok_or("コネクタ一覧が不正です")?{
  let state=c["state"].as_str().unwrap_or("UNKNOWN");
  let ready=["CONNECTED","READ_VERIFIED"].contains(&state);
  let capabilities=if ready{crate::connectors::perform(e,crate::connectors::Action::GetCapabilities{connector_id:c["id"].as_str().ok_or("コネクタIDがありません")?.into()})?}else{Value::Null};
  checks.push(json!({"connectorId":c["id"],"revision":c["revision"],"providerId":c["providerId"],"transport":c["transport"],"connectionState":state,"scope":c["scope"],"capabilityQuery":if ready{"RETURNED"}else{"NOT_READY"},"capabilities":capabilities}));
 }
 let latest=crate::connectors::perform(e,crate::connectors::Action::Get)?;
 if latest!=inventory{return Err("接続情報が確認中に変わりました。投稿せず停止します".into())}
 let reached=checks.iter().any(|v|v["capabilityQuery"]=="RETURNED");
 let result=json!({"version":1,"jobId":j.job_id,"postSha256":post.sha256,"checkedAt":now(),"mode":"CONNECTOR_CHECK_ONLY","status":if checks.is_empty(){"LOCAL_ONLY_NO_CONNECTION"}else if reached{"CONNECTOR_BOUNDARY_REACHED"}else{"CONNECTOR_NOT_READY"},"connectors":checks,"localOutput":"SAVED","remoteReachability":"NOT_VERIFIED","postingExecutor":"NOT_IMPLEMENTED","publication":"NOT_PERFORMED","networkRequests":0,"externalWrites":0});
 e.write_artifact(n,"connector-check.json",&serde_json::to_vec_pretty(&result).map_err(|e|e.to_string())?,"connector-check")?;
 e.update(|s|{s.event(Some("delivery"),"connector_boundary_checked","running",if reached{"接続済みコネクタの機能照会入口へ到達。今回の範囲は確認のみで、投稿は実行していません。"}else{"利用可能なSNS接続なし。投稿文をローカルへ保存し、外部投稿は実行していません。"},None,vec![]);Ok(())})
}
