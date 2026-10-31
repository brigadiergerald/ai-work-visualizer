//! HTML syntax/safety and ID graph validation, separate from product content QA.
use html5ever::tokenizer::{BufferQueue,Token,TokenSink,TokenSinkResult,Tokenizer,TokenizerOpts,StartTag,EndTag};
use std::{cell::RefCell,collections::HashMap};
const MAX_HTML_DEPTH:usize=128;
const MAX_HTML_IDS:usize=4096;
const MAX_HTML_TOKENS:usize=100_000;
// ARIA needs only a nonempty descendant-text predicate, never a copy of that text.
struct Element{tag:String,id:Option<String>,has_text:bool}
#[derive(Default)]pub struct Document{pub text:String,pub sections:usize,pub items:usize,pub images:Vec<String>,body:bool,body_seen:bool,stack:Vec<Element>,css:String,errors:Vec<String>,ja:bool,utf8:bool,viewport:bool,title:bool,h1:bool,ids:HashMap<String,bool>,refs:Vec<String>,tokens:usize,limit_exceeded:bool}
#[derive(Default)]struct Sink(RefCell<Document>,Vec<String>);
impl TokenSink for Sink{
 type Handle=();
 fn process_token(&self,t:Token,_:u64)->TokenSinkResult<()>{
  let mut p=self.0.borrow_mut();
  // Latch exhaustion and stop retaining state/errors for all remaining tokens.
  if p.limit_exceeded{return TokenSinkResult::Continue}
  if p.tokens>=MAX_HTML_TOKENS{p.limit_exceeded=true;return TokenSinkResult::Continue}p.tokens+=1;
  match t{
   Token::TagToken(t)=>{
    let tag=t.name.to_string();
    if !["html","head","meta","title","style","body","main","header","footer","section","h1","h2","h3","p","ul","ol","li","div","span","strong","em","small","br","hr","time","dl","dt","dd","img","figure","figcaption"].contains(&tag.as_str()){p.errors.push(format!("許可外のHTMLタグ: {tag}"));return TokenSinkResult::Continue}
    if t.kind==StartTag{
     if !["meta","br","hr","img"].contains(&tag.as_str())&&p.stack.len()>=MAX_HTML_DEPTH{p.limit_exceeded=true;return TokenSinkResult::Continue}
     let parent=p.stack.last().map(|x|x.tag.as_str());
     let placement=match tag.as_str(){"html"=>parent.is_none()&&!p.body_seen,"head"=>parent==Some("html")&&!p.body_seen,"body"=>parent==Some("html"),"meta"|"title"|"style"=>parent==Some("head"),_=>p.body};
     if !placement{p.errors.push(format!("HTMLの配置が不正: {tag}"))}
     if tag=="html"{p.ja=t.attrs.iter().any(|a|a.name.local.as_ref()=="lang"&&a.value.as_ref()=="ja")}
     if tag=="meta"{
      p.utf8|=t.attrs.iter().any(|a|a.name.local.as_ref()=="charset"&&a.value.eq_ignore_ascii_case("utf-8"));
      p.viewport|=t.attrs.iter().any(|a|a.name.local.as_ref()=="name"&&a.value.as_ref()=="viewport")&&t.attrs.iter().any(|a|a.name.local.as_ref()=="content"&&a.value.contains("width=device-width"));
     }
     if tag=="title"{p.title=true}if tag=="h1"&&p.body{p.h1=true}
     if tag=="img"{
      let src=t.attrs.iter().find(|a|a.name.local.as_ref()=="src").map(|a|a.value.to_string()).unwrap_or_default();
      if !self.1.contains(&src){p.errors.push("同一Jobで承認済みの相対画像参照が必要です".into())}
      if !t.attrs.iter().any(|a|a.name.local.as_ref()=="alt"){p.errors.push("画像のalt属性が必要です".into())}
      p.images.push(src);
     }
     let mut id=None;
     for a in &t.attrs{
      let key=a.name.local.as_ref();let value=a.value.as_ref();
      let allowed=match key{
       "class"=>true,"id"=>!value.is_empty()&&!value.chars().any(|c|c.is_whitespace()||c.is_control()),
       "style"=>{if let Err(e)=crate::css_policy::validate(value,true){p.errors.push(e)}true},
       "lang"=>tag=="html"&&value=="ja","charset"=>tag=="meta"&&value.eq_ignore_ascii_case("utf-8"),
       "name"=>tag=="meta"&&value=="viewport","content"=>tag=="meta"&&t.attrs.iter().any(|a|a.name.local.as_ref()=="name"&&a.value.as_ref()=="viewport"),
       "datetime"=>tag=="time","aria-label"=>!value.trim().is_empty(),
       "src"=>tag=="img"&&self.1.iter().any(|s|s==value),"alt"=>tag=="img",
       "width"|"height"=>tag=="img"&&value.parse::<u32>().is_ok_and(|v|v>0&&v<=8192),
       "loading"=>tag=="img"&&["lazy","eager"].contains(&value),"decoding"=>tag=="img"&&["async","sync","auto"].contains(&value),
       "aria-labelledby"|"aria-describedby"=>{p.refs.extend(value.split_whitespace().map(String::from));!value.trim().is_empty()},
       "role"=>["main","region","banner","contentinfo","note","list","listitem","document","heading"].contains(&value),
       _=>false,
      };
      if !allowed||a.name.prefix.is_some()||!a.name.ns.is_empty(){p.errors.push(format!("許可外のHTML属性/値: {key}"))}
      if key=="id"{if p.ids.len()>=MAX_HTML_IDS{p.limit_exceeded=true;return TokenSinkResult::Continue}if p.ids.insert(value.to_string(),false).is_some(){p.errors.push(format!("重複ID: {value}"))}id=Some(value.to_string());}
     }
     if tag=="body"{if p.body_seen{p.errors.push("bodyが重複".into())}p.body=true;p.body_seen=true;}
     if p.body&&tag=="section"{p.sections+=1}if p.body&&tag=="li"{p.items+=1}
     if !["meta","br","hr","img"].contains(&tag.as_str()){if t.self_closing{p.errors.push("非voidタグの自己終了".into())}p.stack.push(Element{tag,id,has_text:false});}
    }else if t.kind==EndTag{
     if let Some(element)=p.stack.pop(){
      if element.tag!=tag{p.errors.push(format!("閉じタグの不整合: {tag}"))}
      if let Some(id)=element.id{if let Some(has_text)=p.ids.get_mut(&id){*has_text|=element.has_text;}}
      if let Some(parent)=p.stack.last_mut(){parent.has_text|=element.has_text;}
     }else{p.errors.push(format!("閉じタグの不整合: {tag}"))}
     if tag=="body"{p.body=false;}
    }
   },
   Token::CharacterTokens(text)=>{
    if p.stack.last().is_some_and(|x|x.tag=="style"){p.css.push_str(&text);}
    else if p.body{
     p.text.push_str(&text);p.text.push(' ');
     if !text.trim().is_empty(){if let Some(element)=p.stack.last_mut(){element.has_text=true;}}
    }
   },
   Token::ParseError(e)=>p.errors.push(format!("HTML解析エラー: {e}")),
   Token::NullCharacterToken=>p.errors.push("HTMLのNUL文字".into()),_=>{}
  }
  TokenSinkResult::Continue
 }
}
pub fn inspect(raw:&str)->Result<Document,String>{
 inspect_with_images(raw,&[])
}
/// Paths must first be validated against the current Job's material manifest.
pub fn inspect_with_images(raw:&str,images:&[String])->Result<Document,String>{
 if raw.len()>2*1024*1024{return Err("完成HTMLの境界・サイズが不正です".into())}
 let lower=raw.to_ascii_lowercase();
 if !lower.trim_start().starts_with("<!doctype html>")||!lower.trim_end().ends_with("</html>"){return Err("完成HTMLの境界・サイズが不正です".into())}
 let input=BufferQueue::default();input.push_back(raw.into());let tok=Tokenizer::new(Sink(RefCell::new(Document::default()),images.to_vec()),TokenizerOpts::default());let _=tok.feed(&input);tok.end();
 let p=tok.sink.0.into_inner();
 if p.limit_exceeded{return Err("HTMLの深さ・ID数・解析量が上限を超えています".into())}
 if !p.errors.is_empty(){return Err(p.errors.join(" / "))}
 crate::css_policy::validate(&p.css,false)?;
 if !p.stack.is_empty(){return Err("整合した閉じタグが必要です".into())}
 for id in &p.refs{if p.ids.get(id)!=Some(&true){return Err(format!("未定義または空のARIA参照: {id}"))}}
 Ok(p)
}
pub fn require_structure(p:&Document)->Result<(),String>{
 if !p.ja||!p.utf8||!p.viewport||!p.title||!p.h1{return Err("日本語・UTF-8・viewport・title・本文h1が必要です".into())}
 if !p.body_seen||p.sections<2||p.items<3{return Err("body・2 section・3 liが必要です".into())}Ok(())
}
pub fn require_document_structure(p:&Document)->Result<(),String>{
 if !p.ja||!p.utf8||!p.viewport||!p.title||!p.h1||!p.body_seen||p.sections<1{return Err("日本語・UTF-8・viewport・title・本文h1・sectionが必要です".into())}Ok(())
}

#[cfg(test)]mod security_tests{
 use super::*;
 fn document(body:&str)->String{format!("<!doctype html><html lang=\"ja\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>Check</title><style>p{{color:#123}}</style></head><body><h1>Check</h1><section>{body}</section></body></html>")}
 #[test]fn security_html_depth_is_bounded_with_or_without_ids(){
  for ids in [false,true]{
   for (depth,accepted) in [(125,true),(126,false),(256,false)]{
    let mut body=String::new();for n in 0..depth{body.push_str(&if ids{format!("<div id=\"a{n}\">")}else{"<div>".into()});}body.push_str(&"x".repeat(4096));body.push_str(&"</div>".repeat(depth));
    let raw=document(&body);assert_eq!(inspect(&raw).is_ok(),accepted,"depth={depth}, ids={ids}");assert_eq!(inspect_with_images(&raw,&[]).is_ok(),accepted);
   }
  }
 }
 #[test]fn security_html_id_count_is_bounded(){
  for (count,accepted) in [(4096,true),(4097,false)]{
   let body=(0..count).map(|n|format!("<div id=\"a{n}\">x</div>")).collect::<String>();assert_eq!(inspect(&document(&body)).is_ok(),accepted);
  }
 }
 #[test]fn security_html_token_work_is_bounded(){
  assert!(inspect(&document(&"<br>".repeat(1000))).is_ok());
  let raw=document(&"<br>".repeat(100_001));assert!(inspect(&raw).is_err());assert!(inspect_with_images(&raw,&[]).is_err());
 }
 #[test]fn security_html_descendant_text_and_aria_remain_valid(){
  let raw=document("<p aria-labelledby=\"outer leaf\">Reference</p><div id=\"outer\"><div><span id=\"leaf\">&#x78;&#160;</span></div></div><p style=\"color:#123\">Visible</p>");
  let doc=inspect(&raw).unwrap();require_document_structure(&doc).unwrap();assert!(doc.text.contains('x'));assert!(doc.text.contains("Visible"));
  for target in ["<div id=\"target\"> \t&#160;<!--comment--></div>","<div id=\"target\" aria-label=\"not body text\"></div>","<br id=\"target\">","<img id=\"target\" src=\"assets/a.png\" alt=\"not body text\">"]{
   let raw=document(&format!("{target}<p aria-labelledby=\"target\">Reference</p>"));assert!(inspect_with_images(&raw,&["assets/a.png".into()]).is_err());
  }
  for raw in [document("<div id=\"same\">a</div><p id=\"same\">b</p>"),document("<p aria-describedby=\"missing\">Reference</p>"),document("<div id=\"bad id\">x</div>"),document("<div>x</span>"),document("<p style=\"display:none\">hidden</p>")]{assert!(inspect(&raw).is_err());}
  let head_only=raw.replace("<title>Check</title>","<title id=\"head\">Check</title>").replace("Reference</p>","Reference</p><p aria-labelledby=\"head\">x</p>");assert!(inspect(&head_only).is_err());
  let style_only=raw.replace("<style>","<style id=\"style-only\">").replace("Reference</p>","Reference</p><p aria-labelledby=\"style-only\">x</p>");assert!(inspect(&style_only).is_err());
 }
}
