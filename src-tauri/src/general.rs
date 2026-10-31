//! Generic, host-written delivery. Two Codex turns; deterministic QA/save only.
//! No browser, shared personal registry, arbitrary code execution or package install.
use crate::{model::*,practical::{Plan,Practical,Quality}};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::{fs,path::Path,collections::HashSet};
pub const ID:&str="codex-general-v1";
pub fn applies(j:&Job)->bool{j.workflow_id==ID||crate::sns_team::applies(j)}
fn sensitive_input(text:&str)->bool{
 fn credentials(v:&serde_json::Value)->bool{match v{serde_json::Value::Object(o)=>o.iter().any(|(k,v)|{let key=k.to_ascii_lowercase();(["api_key","apikey","access_token","refresh_token","password","client_secret","authorization","cookie"].iter().any(|s|key.contains(s))&&!v.is_null()&&v.as_str()!=Some(""))||credentials(v)}),serde_json::Value::Array(a)=>a.iter().any(credentials),_=>false}}
 redact(text)!=text||serde_json::from_str::<serde_json::Value>(text).is_ok_and(|v|credentials(&v))
}
pub fn inputs(settings:&crate::setup::Settings)->Result<Vec<(String,Vec<u8>)>,String>{
 if settings.input_paths.len()>12{return Err("入力ファイルは12件までです".into())}
 let root=Path::new(&settings.work_root).canonicalize().map_err(|e|e.to_string())?;let mut result=vec![];
 for (i,path) in settings.input_paths.iter().enumerate(){let p=Path::new(path).canonicalize().map_err(|e|format!("入力ファイルがありません: {e}"))?;
  let name=p.file_name().and_then(|v|v.to_str()).ok_or("入力名が不正です")?;
  if !p.starts_with(&root)||!p.is_file()||!safe_name(name)||["auth.json","credentials.json","secrets.json"].contains(&name.to_lowercase().as_str()){return Err("入力は作業範囲内の通常の文書/データ/ソースファイルを指定してください。認証ファイルは取り込めません".into())}
  if fs::metadata(&p).map_err(|e|e.to_string())?.len()>2*1024*1024{return Err("入力は1ファイル2MBまでです".into())}
  let bytes=fs::read(&p).map_err(|e|e.to_string())?;let text=std::str::from_utf8(&bytes).map_err(|_|"入力ファイルはUTF-8テキストが必要です")?;
  if sensitive_input(text){return Err("入力に認証情報らしい値があります。秘密を除いたファイルを指定してください".into())}
  result.push((format!("input-{i:02}-{name}"),bytes));
 }Ok(result)
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct FileSpec{pub name:String,pub required_text:Vec<String>}
#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Contract{pub title:String,pub requirements:Vec<String>,pub checklist:Vec<String>,#[serde(default)]pub production_constraints:Vec<String>,pub files:Vec<FileSpec>,#[serde(default)]pub test_command:String}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File{pub name:String,pub content:String}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle{files:Vec<File>}
pub fn safe_name(name:&str)->bool{
 let p=Path::new(name);let stem=p.file_stem().and_then(|v|v.to_str()).unwrap_or("").to_ascii_uppercase();
 !name.is_empty()&&name.len()<100&&!name.contains(['/', '\\', ':','<','>','"','|','?','*'])&&!name.contains("..")&&!name.ends_with([' ','.'])&&!name.chars().any(char::is_control)
 &&!matches!(stem.as_str(),"CON"|"PRN"|"AUX"|"NUL"|"COM1"|"COM2"|"COM3"|"COM4"|"COM5"|"COM6"|"COM7"|"COM8"|"COM9"|"LPT1"|"LPT2"|"LPT3"|"LPT4"|"LPT5"|"LPT6"|"LPT7"|"LPT8"|"LPT9")
 &&!matches!(name.to_ascii_lowercase().as_str(),"plan.json"|"bundle.json"|"quality.json"|"manifest.json"|"run-tests.json")
 &&p.extension().and_then(|v|v.to_str()).is_some_and(|e|["md","txt","html","json","csv","tsv","py","js","mjs","ts","tsx","rs","css"].contains(&e.to_ascii_lowercase().as_str()))
}
pub fn intake(input:&str)->Result<Option<Practical>,String>{intake_for(input,true)}
pub fn intake_generic(input:&str)->Result<Option<Practical>,String>{intake_for(input,false)}
fn intake_for(input:&str,allow_legacy:bool)->Result<Option<Practical>,String>{
 if input.trim().is_empty()||input.len()>64000{return Err("指示は1〜64,000バイトで入力してください".into())}
 // The established event/document contracts remain byte-compatible.
 let mut requested=input.to_lowercase();
 for subject in ["画像生成","画像","挿絵","イラスト","chatgpt","外部調査","ウェブ検索","公開","投稿","動画","音声"]{
  for suffix in ["なし","無し","不要","は不要","を使わない","は使わない","を生成しない","はしない","しない"]{requested=requested.replace(&format!("{subject}{suffix}"),"");}
 }
 // Do not promise tools that this product does not have. Users can describe
 // those actions in a document by explicitly saying they are not being done.
 if ["画像を生成","画像を作","画像が必須","画像を含む","挿絵","イラスト","chatgptへ","通常chatgpt","動画を","音声を","pdfで","pptx","xlsx","外部調査","ウェブ検索","投稿して","公開して","購入して","削除して","ブラウザで","generate an image","generate images","generate a video","post to","publish to","delete files"].iter().any(|s|requested.contains(s)){
  return Err("この版はローカル文書・データ・ソース制作に対応します。画像/動画生成・外部サービス操作・公開・購入・既存ファイル削除は追加のツールと権限が必要で、実行しません。".into())
 }
 if allow_legacy&&crate::codex_only::legacy(input){return crate::practical::intake(input)}
 Ok(Some(Practical{version:5,original_instruction:input.into(),turn_starts:0,max_turn_starts:2,plan:None}))
}
pub fn workflow()->Workflow{
 let defs=[
 ("plan","調査・要件整理","scholar","codex",vec![],vec![],"plan.json","ユーザー原文・添付原文から完成品の要件を決める。JSONだけ: {\"title\":文字列,\"requirements\":[完成品全体の必須内容],\"checklist\":[受入条件],\"productionConstraints\":[制作条件・不明点],\"files\":[{\"name\":安全なファイル名,\"requiredText\":[そのファイル本文に必須の原文文字列]}]}。filesは1〜12件、平坦な名前、拡張子md/txt/html/json/csv/tsv/py/js/mjs/ts/tsx/rs/cssのみ。plan.json/bundle.json/quality.json/manifest.jsonは予約名。原文の目的・対象・形式・正確な表示文言を維持。重大な不足は創作せず報告して停止。本文へ内部管理情報を混入させない。コードでは利用手順とテスト用ソースを必要に応じ計画する。テスト要求があるPythonはtestCommand=python-unittestとtest_*.py、Node.jsはtestCommand=node-testと*.test.mjsを計画する（JSONのトップレベルにtestCommandを追加）。ホストがCodex sandboxで検査する。実行テストの要求がなければtestCommandは空文字。対応外の言語の実行テストは未対応として不足を報告。"),
 ("writing","計画・設計／制作・実装","artist","codex",vec!["plan"],vec!["plan.json"],"bundle.json","plan.jsonとユーザー原文に沿って使える完成品を制作。JSONだけ: {\"files\":[{\"name\":planの出力名,\"content\":ファイルの完全な内容}]}。計画した全ファイルを過不足なく返す。コード囲みや付随報告なし。requiredTextと原文の表示文言を保持。HTMLは自己完結の静的文書、lang=ja/UTF-8/viewport/title/h1/sectionを含み、外部URL/画像/script/フォームは禁止。JSONは有効なJSON、CSV/TSVは同じ列数で表計算式を含めない。ソースはコードをそのまま納品。コードの実行テスト/依存install/外部操作を実行したと主張しない。未確認の数値・体験・引用を創作しない。"),
 ("qa","品質検査・修正確認","analyst","quality",vec!["writing"],vec!["plan.json","bundle.json"],"quality.json","全納品ファイル・原文指定・形式・所属・hashを検査。不一致は停止。意味・表示・コード実行テストは機械検査とは別判定。"),
 ("delivery","保存・納品","messenger","save",vec!["qa"],vec!["bundle.json","quality.json"],"manifest.json","完成品を再検証し、所属とSHA-256を保存する。公開・自動更新・外部操作なし。")];
 let nodes=defs.into_iter().map(|(id,label,sprite,executor,deps,inputs,output,work)|Node{node_id:id.into(),role:if executor=="codex"{"Codex".into()}else{"検証・保存".into()},label:label.into(),sprite_key:sprite.into(),executor:executor.into(),dependencies:deps.into_iter().map(String::from).collect(),input_artifacts:inputs.into_iter().map(String::from).collect(),output_artifacts:vec![output.into()],work_instruction:work.into()}).collect::<Vec<_>>();
 let edges=nodes.iter().flat_map(|n|n.dependencies.iter().map(|d|Edge{edge_id:format!("{d}-{}",n.node_id),from_node_id:d.clone(),to_node_id:n.node_id.clone()})).collect();
 Workflow{workflow_id:ID.into(),workflow_name:"Codexで仕事を完成させる".into(),nodes,edges}
}
fn contract(raw:&str,input:&str)->Result<Contract,String>{
 let c:Contract=serde_json::from_str(raw).map_err(|e|format!("汎用計画JSONが不正です: {e}"))?;
 if c.title.trim().is_empty()||c.requirements.is_empty()||c.files.is_empty()||c.files.len()>12{return Err("計画には題名・必須内容・1〜12件の成果物が必要です".into())}
 let mut names=HashSet::new();
 for f in &c.files{if !safe_name(&f.name)||!names.insert(f.name.to_lowercase())||f.required_text.iter().any(|s|s.trim().is_empty()){return Err("計画のファイル名・重複・必須文言が不正です".into())}}
 let planned=c.requirements.iter().chain(c.checklist.iter()).chain(c.files.iter().flat_map(|f|f.required_text.iter())).cloned().collect::<Vec<_>>().join("\n");
 for lock in crate::codex_only::exact_copy(input){if !planned.contains(&lock)&&!c.files.iter().any(|f|f.name==lock){return Err(format!("原文の指定文言が計画から欠落: {lock}"))}}
 if !["","python-unittest","node-test"].contains(&c.test_command.as_str()){return Err("テスト方式はpython-unittestまたはnode-testだけを使用できます。任意コマンドや依存installは実行しません".into())}
 if (input.contains("テスト")||input.to_lowercase().contains("run tests"))&&c.files.iter().any(|f|["py","js","mjs","ts","tsx","rs"].iter().any(|ext|f.name.ends_with(&format!(".{ext}"))))&&c.test_command.is_empty(){return Err("要求された実行テストの方式が計画にありません。Python unittest / Node.js node:test以外は実行未対応です".into())}
 if c.test_command=="python-unittest"&&!c.files.iter().any(|f|f.name.starts_with("test_")&&f.name.ends_with(".py")){return Err("Pythonテストにはtest_*.pyが必要です".into())}
 if c.test_command=="node-test"&&!c.files.iter().any(|f|f.name.ends_with(".test.mjs")){return Err("Nodeテストには*.test.mjsが必要です".into())}
 Ok(c)
}
pub fn parse_plan(raw:&str,input:&str)->Result<Plan,String>{let c=contract(raw,input)?;Ok(Plan{title:c.title,requirements:c.requirements,checklist:c.checklist,production_constraints:c.production_constraints})}
fn csv_valid(raw:&str,separator:char)->Result<(),String>{
 // Consume the optional encoding marker for inspection only; product bytes stay unchanged.
 let raw=raw.strip_prefix('\u{feff}').unwrap_or(raw);
 let mut fields=vec![];let mut row=vec![];let mut cell=String::new();let mut quoted=false;let mut closed=false;let mut chars=raw.chars().peekable();
 while let Some(c)=chars.next(){if quoted{if c=='"'{if chars.peek()==Some(&'"'){chars.next();cell.push('"')}else{quoted=false;closed=true}}else{cell.push(c)}}else{match c{
  '"' if cell.is_empty()&&!closed=>quoted=true,
  x if x==separator=>{row.push(std::mem::take(&mut cell));closed=false},
  '\r'|'\n'=>{if c=='\r'&&chars.peek()==Some(&'\n'){chars.next();}row.push(std::mem::take(&mut cell));fields.push(std::mem::take(&mut row));closed=false},
  _=>{if closed||c=='"'{return Err("CSVの引用符・区切りが不正です".into())}cell.push(c)}
 }}}
 if quoted{return Err("CSVの引用符が閉じていません".into())}
 if !cell.is_empty()||!row.is_empty()||closed{row.push(cell);fields.push(row)}
 if fields.is_empty()||fields[0].is_empty()||fields.iter().any(|r|r.len()!=fields[0].len()){return Err("CSV/TSVが空、または列数が一致しません".into())}
 if fields.iter().flatten().any(|v|{let visible=v.trim_start_matches(|c:char|c.is_whitespace()||c=='\u{feff}');visible.starts_with(['=','+','-','@'])&&!visible.trim().parse::<f64>().is_ok()}){return Err("CSV/TSVの表計算式は安全に開けないため拒否しました".into())}Ok(())
}
pub fn inspect_output(j:&Job,raw:&str)->Result<Vec<File>,String>{
 let p=j.artifacts.iter().find(|a|a.node_id=="plan"&&a.name=="plan.json").ok_or("計画ファイルがありません")?;crate::engine::verify_artifact(p,Path::new(&j.workspace))?;
 let c=contract(&crate::decisions::production_payload(j,&fs::read_to_string(&p.path).map_err(|e|e.to_string())?),&j.instruction)?;
 if raw.len()>8*1024*1024{return Err("成果物bundleが8MBを超えています".into())}
 let payload=crate::decisions::production_payload(j,raw);let b:Bundle=serde_json::from_str(&payload).map_err(|e|format!("納品bundle JSONが不正です: {e}"))?;
 if b.files.len()!=c.files.len(){return Err("計画と納品ファイル数が一致しません".into())}
 let mut seen=HashSet::new();
 for f in &b.files{
  let spec=c.files.iter().find(|s|s.name==f.name).ok_or("計画にないファイルです")?;
  if !safe_name(&f.name)||!seen.insert(f.name.to_lowercase())||f.content.trim().is_empty()||f.content.len()>2*1024*1024||f.content.chars().any(|c|c.is_control()&&!['\n','\r','\t'].contains(&c)){return Err("納品ファイルの名前・内容・サイズが不正です".into())}
  let ext=Path::new(&f.name).extension().unwrap().to_string_lossy().to_lowercase();
  let text=if ext=="html"{let d=crate::html_policy::inspect(&f.content)?;crate::html_policy::require_document_structure(&d)?;d.text}else{f.content.clone()};
  if ext=="json"{let _:serde_json::Value=serde_json::from_str(&f.content).map_err(|e|e.to_string())?;}
  if ext=="csv"||ext=="tsv"{csv_valid(&f.content,if ext=="csv"{','}else{'\t'})?;}
  for lock in &spec.required_text{if !text.contains(lock){return Err(format!("{}の必須内容が欠落: {lock}",f.name))}}
 }
 let all=b.files.iter().map(|f|if f.name.to_lowercase().ends_with(".html"){crate::html_policy::inspect(&f.content).map(|d|d.text)}else{Ok(f.content.clone())}).collect::<Result<Vec<_>,String>>()?.join("\n");
 for lock in crate::codex_only::exact_copy(&j.instruction){if !all.contains(&lock)&&!b.files.iter().any(|f|f.name==lock){return Err(format!("原文の必須文言が納品から欠落: {lock}"))}}
 Ok(b.files)
}
pub fn verify_product(j:&Job)->Result<Quality,String>{
 if crate::sns_team::applies(j){let research=j.artifacts.iter().filter(|a|a.node_id=="research"&&a.name=="research.json").collect::<Vec<_>>();if research.len()!=1{return Err("調査成果物が欠落・重複しています".into())}crate::engine::verify_artifact(research[0],Path::new(&j.workspace))?;crate::sns_team::validate_research(&fs::read_to_string(&research[0].path).map_err(|e|e.to_string())?)?;}
 for a in &j.artifacts{crate::engine::verify_artifact(a,Path::new(&j.workspace))?;}
 let bundles=j.artifacts.iter().filter(|a|a.node_id=="writing"&&a.name=="bundle.json").collect::<Vec<_>>();if bundles.len()!=1{return Err("納品bundleが欠落・重複しています".into())}
 let raw=fs::read_to_string(&bundles[0].path).map_err(|e|e.to_string())?;let files=inspect_output(j,&raw)?;
 for f in &files{let matches=j.artifacts.iter().filter(|a|a.node_id=="writing"&&a.kind=="product"&&a.name==f.name).collect::<Vec<_>>();if matches.len()!=1||fs::read(&matches[0].path).map_err(|e|e.to_string())?!=f.content.as_bytes(){return Err("bundleと実納品ファイルが一致しません".into())}}
 if j.artifacts.iter().filter(|a|a.kind=="product").count()!=files.len(){return Err("納品所属または件数が不一致です".into())}
 let c=read_contract(j)?;if !c.test_command.is_empty(){let records=j.artifacts.iter().filter(|a|a.kind=="test"&&a.name=="run-tests.json").collect::<Vec<_>>();if records.len()!=1{return Err("要求されたコード実行テストの記録がありません".into())}let r:serde_json::Value=serde_json::from_slice(&fs::read(&records[0].path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;if r["productDigest"]!=product_digest(j)||r["testCommand"]!=c.test_command||r["response"]["exitCode"]!=0{return Err("実行テストが未合格、または納品fileと一致しません".into())}}
 Ok(Quality{artifact_sha256:format!("{:x}",Sha256::digest(raw.as_bytes())),machine_status:"PASS".into(),display_status:"NOT_VERIFIED".into(),checked_at:now(),checks:vec!["independent original quoted copy + per-file acceptance".into(),"exact planned files + registered ownership + full bytes/SHA-256".into(),"JSON/CSV/TSV/static HTML format where applicable".into(),"semantic usefulness/display: NOT_VERIFIED".into(),if c.test_command.is_empty(){"runtime code tests: NOT_REQUESTED/NOT_VERIFIED".into()}else{format!("selected runtime test {}: exit 0; product digest verified",c.test_command)}]})
}
fn read_contract(j:&Job)->Result<Contract,String>{let p=j.artifacts.iter().find(|a|a.node_id=="plan"&&a.name=="plan.json").ok_or("元計画なし")?;crate::engine::verify_artifact(p,Path::new(&j.workspace))?;contract(&crate::decisions::production_payload(j,&fs::read_to_string(&p.path).map_err(|e|e.to_string())?),&j.instruction)}
fn product_digest(j:&Job)->String{let mut files=j.artifacts.iter().filter(|a|a.kind=="product").collect::<Vec<_>>();files.sort_by(|a,b|a.name.cmp(&b.name));let mut hash=Sha256::new();for f in files{hash.update(f.name.as_bytes());hash.update([0]);hash.update(f.sha256.as_bytes());hash.update([0]);}format!("{:x}",hash.finalize())}
pub fn run_tests(engine:&crate::engine::Engine,j:&Job,n:&Node)->Result<(),String>{
 let c=read_contract(j)?;if c.test_command.is_empty(){return Ok(())}
 for a in &j.artifacts{crate::engine::verify_artifact(a,Path::new(&j.workspace))?;}
 if j.artifacts.iter().any(|a|a.kind=="test"){verify_product(j)?;return Ok(())}
 if engine.snapshot().events.iter().any(|e|e.job_id==j.job_id&&e.event_type=="code_test_unknown"){return Err("元の実行テスト結果はUNKNOWNです。自動再実行せず、元app-serverの状態を確認してください".into())}
 let mut command=if c.test_command=="python-unittest"{vec!["python.exe".to_string(),"-I".into(),"-B".into(),"-m".into(),"unittest".into(),"discover".into(),"-s".into(),j.workspace.clone(),"-p".into(),"test_*.py".into()]}else{vec!["node.exe".into(),"--test".into()]};
 if c.test_command=="node-test"{command.extend(c.files.iter().filter(|f|f.name.ends_with(".test.mjs")).map(|f|Path::new(&j.workspace).join(&f.name).to_string_lossy().into_owned()));}
 let params=serde_json::json!({"command":command,"cwd":j.workspace,"sandboxPolicy":{"type":"readOnly","access":{"type":"restricted","includePlatformDefaults":true,"readableRoots":[j.workspace]}},"timeoutMs":20000});
 engine.update(|s|{s.event(Some(&n.node_id),"code_test_requested","running",format!("{}：Codex sandboxで実行テスト。新しい推論・依存install・追加権限は使用しません。",c.test_command),None,vec![]);Ok(())})?;
 let response=engine.get_rpc()?.call("command/exec",params.clone());
 let result=match response{Ok(r)=>r,Err(error)=>{engine.update(|s|{s.event(Some(&n.node_id),"code_test_unknown","failed",format!("実行テストの結果を確認できません。自動再実行しません: {error}"),None,vec![]);Ok(())})?;return Err(error)}};
 let report=serde_json::json!({"testCommand":c.test_command,"productDigest":product_digest(j),"request":params,"response":{"exitCode":result["exitCode"],"stdout":redact(result["stdout"].as_str().unwrap_or("")),"stderr":redact(result["stderr"].as_str().unwrap_or(""))},"checkedAt":now(),"inferencePerformed":false});
 engine.write_artifact(n,"run-tests.json",&serde_json::to_vec_pretty(&report).map_err(|e|e.to_string())?,"test")?;
 if result["exitCode"]!=0{return Err("コード実行テストが失敗しました。元の結果を保持し、自動修正推論は開始しません。".into())}Ok(())
}

#[cfg(test)]mod csv_security_tests{
 use super::csv_valid;
 #[test]fn security_csv_bom_formula_variants_are_rejected(){
  for separator in [',','\t']{for formula in ["=1+1","+SUM(1)","-SUM(1)","@SUM(1)"]{
   for raw in [format!("\u{feff}{formula}\n"),format!("\u{feff}\"{formula}\"\n"),format!("\u{feff}\u{feff} \t{formula}\n"),format!("header{separator}\" \u{feff} {formula}\"\n"),format!("header\n\"\u{feff}{formula}\"\n")]{
    assert!(csv_valid(&raw,separator).is_err(),"accepted formula variant: {raw:?}");
   }
  }}
  let escaped:String=serde_json::from_str(r#""\uFEFF=1+1""#).unwrap();assert!(csv_valid(&escaped,',').is_err());
 }
 #[test]fn security_csv_bom_and_ordinary_data_remain_valid(){
  for separator in [',','\t']{
   for raw in [format!("\u{feff}\"name\"{separator}\"value\"\r\n\"item\"{separator}-3.5\r\n"),format!("\"a{separator}b\"{separator}\"quoted \"\"text\"\"\"\n"),format!("\"line one\nline two\"{separator}+2e3\n"),format!("'text{separator}\u{feff}ordinary\n"),format!("\u{feff}-3{separator}\" \u{feff}+2\"\n")]{
    assert!(csv_valid(&raw,separator).is_ok(),"rejected ordinary data: {raw:?}");
   }
   for raw in [format!("\u{feff}\"not closed{separator}x"),format!("\"a\"x{separator}b"),format!("a{separator}b\nc\n")]{assert!(csv_valid(&raw,separator).is_err());}
  }
 }
}
