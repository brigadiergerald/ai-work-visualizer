//! Output schema for the autonomous SNS workflow. Host validation stays strict.
use serde_json::{json,Value};
pub fn for_job(j:&crate::model::Job,node:&str)->Option<Value>{
 let mut s=schema(node)?;
 if j.workflow_id==crate::sns_team::DECISIONS_ID&&["plan","writing"].contains(&node){
  let note=json!({"type":"object","additionalProperties":false,"properties":{"summary":{"type":"string"},"reason":{"type":"string"},"details":{"type":"array","items":{"type":"string"}}},"required":["summary","reason","details"]});
  let keys=if node=="plan"{vec!["theme","direction"]}else{vec!["writing"]};let mut props=serde_json::Map::new();for key in &keys{props.insert(key.to_string(),note.clone());}
  s["properties"]["decisionNotes"]=json!({"type":"object","additionalProperties":false,"properties":props,"required":keys});s["required"].as_array_mut()?.push(json!("decisionNotes"));
 }Some(s)
}
pub fn schema(node:&str)->Option<Value>{
 let strings=json!({"type":"array","items":{"type":"string"}});
 match node{
  "research"=>Some(crate::sns_team::research_schema()),
  "plan"=>Some(json!({"type":"object","additionalProperties":false,"properties":{"title":{"type":"string"},"requirements":strings,"checklist":strings,"productionConstraints":strings,"files":{"type":"array","minItems":1,"maxItems":1,"items":{"type":"object","additionalProperties":false,"properties":{"name":{"type":"string","enum":["post.txt"]},"requiredText":strings},"required":["name","requiredText"]}},"testCommand":{"type":"string","enum":[""]}},"required":["title","requirements","checklist","productionConstraints","files","testCommand"]})),
  "writing"=>Some(json!({"type":"object","additionalProperties":false,"properties":{"files":{"type":"array","minItems":1,"maxItems":1,"items":{"type":"object","additionalProperties":false,"properties":{"name":{"type":"string","enum":["post.txt"]},"content":{"type":"string"}},"required":["name","content"]}}},"required":["files"]})),
  _=>None
 }
}
