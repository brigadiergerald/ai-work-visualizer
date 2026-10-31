//! Basic Workflow policy. Uses the existing two-turn generic runner.
use crate::{engine::{Engine,verify_artifact},model::*};
use serde_json::{json,Value};
use std::{path::Path,fs};
pub const ID:&str="sns-draft-v1";
pub const REVIEW:&str="sns_review";
pub const AUTONOMOUS_POLICY:&str="今回の利用者指定は、不足情報をAI判断で補い、確認質問で停止せずそのまま文章案を完成させること。媒体未指定・文体・構成・文字数などは目的と対象者に合う妥当な選択で進める。本人の実体験が未提供なら、実体験を必須要件にせず一般的な案内文にする。過去のエピソード必須・素材不足時の質問ルールより、この今回指定を優先する。本人の経験・発言、未確認の効果・数値・引用・出典は事実として創作しない。確定文言と提供事実を保持し、権限や公開範囲は拡張しない。";
pub fn applies(j:&Job)->bool{crate::sns_team::applies(j)||j.workflow.nodes.iter().any(|n|n.node_id==REVIEW&&n.executor=="review")}
pub fn autonomous(j:&Job)->bool{applies(j)&&j.workflow.nodes.iter().any(|n|n.node_id=="plan"&&n.work_instruction.contains(AUTONOMOUS_POLICY))}
pub fn workflow()->Workflow{build_workflow(false)}
pub fn autonomous_workflow()->Workflow{build_workflow(true)}
fn build_workflow(autonomous:bool)->Workflow{
 let mut w=crate::general::workflow();
 w.workflow_name="SNS投稿文の作成・投稿準備".into();
 for n in &mut w.nodes{
  if n.node_id=="plan"{n.work_instruction.push_str("\n用途はSNS投稿文の作成・投稿準備。納品はUTF-8のpost.txt 1件のみ。testCommandは空文字。外部公開・予約・投稿はしない。対象読者、目的、媒体、確定文言、提供された事実・体験を維持し、不明な体験・数値・実績を創作しない。本人のエピソードが必要で素材がなければ質問する。話し言葉、長短の文、余韻のある終わりを基本とし、ユーザー指定を優先する。");}
  if n.node_id=="writing"{n.label="SNS文章案の作成".into();n.work_instruction.push_str("\npost.txtだけを制作する。文章案であり、投稿成功・公開済みとは書かない。未確認の本人の経験は創作せず質問する。話し言葉と長短の文を自然に混ぜ、原文の目的と固定文言を守る。");}
  if autonomous&&n.node_id=="plan"{n.work_instruction=format!("ユーザー原文・提供された素材から、SNS投稿文の完成要件を決める。{AUTONOMOUS_POLICY}\n話し言葉、長短の文、余韻のある終わりを基本に、今回の目的に合わせて判断する。質問待ちや未提供の実体験を制作条件へ追加しない。JSONだけ: {{\"title\":題名,\"requirements\":[完成品の要件],\"checklist\":[受入条件],\"productionConstraints\":[AIが選んだ編集方針・事実上の限界],\"files\":[{{\"name\":\"post.txt\",\"requiredText\":[本文に必須の原文文字列]}}],\"testCommand\":\"\"}}。外部公開・予約・投稿、実行テストは行わない。内部管理情報は本文へ入れない。");}
  if autonomous&&n.node_id=="writing"{n.work_instruction=format!("元計画・原文・提供事実から、読者がそのまま読めるSNS文章案を完成させる。{AUTONOMOUS_POLICY}\n話し言葉と長短の文を自然に混ぜ、原文の目的と固定文言を守る。未提供の実体験は本文に入れず一般的な案内にする。JSONだけ: {{\"files\":[{{\"name\":\"post.txt\",\"content\":完成した投稿本文}}]}}。questionキー、確認依頼、仮の体験、制作上の説明を納品に混入させない。外部公開済みとは書かない。ファイル保存はホストが行う。");}
  if n.node_id=="delivery"{n.dependencies=vec![REVIEW.into()];n.input_artifacts.push("review.json".into());}
 }
 w.nodes.insert(3,Node{node_id:REVIEW.into(),role:"人間".into(),label:"内容確認・人間の承認".into(),sprite_key:"analyst".into(),work_instruction:"文章案の実内容とhashを確認し、納品を承認または停止する。外部公開の承認ではない。".into(),dependencies:vec!["qa".into()],input_artifacts:vec!["post.txt".into(),"quality.json".into()],output_artifacts:vec!["review.json".into()],executor:"review".into()});
 w.edges=w.nodes.iter().flat_map(|n|n.dependencies.iter().map(|d|Edge{edge_id:format!("{d}-{}",n.node_id),from_node_id:d.clone(),to_node_id:n.node_id.clone()})).collect();w
}
pub fn validate_plan(raw:&str)->Result<(),String>{let v:Value=serde_json::from_str(raw).map_err(|e|e.to_string())?;if v["files"].as_array().is_none_or(|f|f.len()!=1||f[0]["name"]!="post.txt")||v["testCommand"].as_str().is_some_and(|s|!s.is_empty()){return Err("SNS文章の出力はpost.txt 1件だけです。実行テスト・投稿は行いません".into())}Ok(())}
fn post(j:&Job)->Result<&Artifact,String>{let a=j.artifacts.iter().find(|a|a.kind=="product"&&a.name=="post.txt").ok_or("文章案がありません")?;verify_artifact(a,Path::new(&j.workspace))?;Ok(a)}
pub fn request(e:&Engine,n:&Node)->Result<(),String>{
 let s=e.snapshot();let j=s.job.as_ref().ok_or("Jobなし")?;crate::general::verify_product(j)?;let a=post(j)?;
 e.update(|s|{let j=s.job.as_mut().unwrap();j.status="awaiting_approval".into();j.turn_id=None;j.nodes.get_mut(REVIEW).unwrap().status="awaiting_approval".into();s.approvals.push(Approval{id:id(),request_id:Value::Null,job_id:j.job_id.clone(),node_id:REVIEW.into(),kind:"content_review".into(),title:"文章案を確認して納品を承認".into(),details:n.work_instruction.clone(),params:json!({"artifactPath":a.path,"sha256":a.sha256}),resolved:false,decision:None});s.event(Some(REVIEW),"content_review_requested","awaiting_approval","文章案の内容確認を待っています。外部への投稿は行っていません。",None,vec![a.path.clone()]);Ok(())})
}
pub fn answer(e:&Engine,job_id:&str,approval_id:&str,decision:&str)->Result<(),String>{
 let s=e.snapshot();let j=s.job.as_ref().filter(|j|j.job_id==job_id&&applies(j)&&j.status=="awaiting_approval"&&!j.stop_requested).ok_or("現在の文章確認Jobと一致しません")?;
 let a=s.approvals.iter().find(|a|a.id==approval_id&&a.job_id==job_id&&a.kind=="content_review"&&!a.resolved&&a.decision.is_none()).ok_or("有効な内容確認がありません。二重承認はしません")?;
 let file=post(j)?;crate::general::verify_product(j)?;if a.params["sha256"]!=file.sha256||a.params["artifactPath"]!=file.path{return Err("承認対象の内容・hashが変わっています".into())}
 if !["accept","decline"].contains(&decision){return Err("承認結果が不正です".into())}
 if decision=="accept"{
  let n=j.workflow.nodes.iter().find(|n|n.node_id==REVIEW).unwrap();
  e.write_artifact(n,"review.json",&serde_json::to_vec_pretty(&json!({"jobId":job_id,"approvalId":approval_id,"sha256":file.sha256,"decision":"accept","scope":"LOCAL_TEXT_DELIVERY_ONLY","publication":"NOT_PERFORMED","timestamp":now()})).unwrap(),"review")?;
 }
 e.update(|s|{let a=s.approvals.iter_mut().find(|a|a.id==approval_id).unwrap();a.decision=Some(decision.into());a.resolved=true;let j=s.job.as_mut().unwrap();j.status="running".into();j.nodes.get_mut(REVIEW).unwrap().status="running".into();s.event(Some(REVIEW),"content_review_answered","running",format!("文章案の納品確認: {decision}。外部投稿なし。"),None,vec![]);Ok(())})?;
 if decision=="decline"{return e.cancelled()}
 e.update(|s|{let j=s.job.as_mut().unwrap();let n=j.nodes.get_mut(REVIEW).unwrap();n.status="completed".into();n.ended_at=Some(now());Ok(())})
}
pub fn recover(e:&Engine,job_id:&str)->Result<(),String>{
 let s=e.snapshot();let j=s.job.as_ref().filter(|j|j.job_id==job_id&&applies(j)&&j.status=="disconnected"&&j.active_node_id.as_deref()==Some(REVIEW)).ok_or("ローカル内容確認の復旧対象ではありません")?;
 crate::general::verify_product(j)?;let file=post(j)?;
 if !s.approvals.iter().any(|a|a.job_id==job_id&&a.kind=="content_review"&&!a.resolved&&a.decision.is_none()&&a.params["sha256"]==file.sha256){return Err("未回答の元承認と実fileを照合できません。再生成しません".into())}
 e.update(|s|{let j=s.job.as_mut().unwrap();j.status="running".into();j.nodes.get_mut(REVIEW).unwrap().status="running".into();Ok(())})?;
 e.update(|s|{let j=s.job.as_mut().unwrap();j.status="awaiting_approval".into();j.nodes.get_mut(REVIEW).unwrap().status="awaiting_approval".into();s.event(Some(REVIEW),"content_review_recovered","awaiting_approval","元の承認と文章案の実file/hashを照合しました。推論・再送なし。",None,vec![]);Ok(())})
}
pub fn approved(j:&Job)->Result<(),String>{
 if crate::sns_team::applies(j){crate::general::verify_product(j)?;return Ok(())}
 if !applies(j){return Ok(())}let file=post(j)?;let a=j.artifacts.iter().find(|a|a.kind=="review"&&a.name=="review.json").ok_or("人間の内容承認が未完了です")?;verify_artifact(a,Path::new(&j.workspace))?;
 let v:Value=serde_json::from_slice(&fs::read(&a.path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 if v["jobId"]!=j.job_id||v["sha256"]!=file.sha256||v["decision"]!="accept"{return Err("内容承認の所属・hashが一致しません".into())}Ok(())
}
