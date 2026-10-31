//! HTML syntax/safety and ID graph validation, separate from product content QA.
use html5ever::tokenizer::{BufferQueue,Token,TokenSink,TokenSinkResult,Tokenizer,TokenizerOpts,StartTag,EndTag};
use std::{cell::RefCell,collections::HashMap};
#[derive(Default)]pub struct Document{pub text:String,pub sections:usize,pub items:usize,pub images:Vec<String>,body:bool,body_seen:bool,stack:Vec<(String,Option<String>)>,css:String,errors:Vec<String>,ja:bool,utf8:bool,viewport:bool,title:bool,h1:bool,ids:HashMap<String,String>,refs:Vec<String>}
#[derive(Default)]struct Sink(RefCell<Document>,Vec<String>);
impl TokenSink for Sink{
 type Handle=();
 fn process_token(&self,t:Token,_:u64)->TokenSinkResult<()>{
  let mut p=self.0.borrow_mut();
  match t{
   Token::TagToken(t)=>{
    let tag=t.name.to_string();
    if !["html","head","meta","title","style","body","main","header","footer","section","h1","h2","h3","p","ul","ol","li","div","span","strong","em","small","br","hr","time","dl","dt","dd","img","figure","figcaption"].contains(&tag.as_str()){p.errors.push(format!("許可外のHTMLタグ: {tag}"));return TokenSinkResult::Continue}
    if t.kind==StartTag{
     let parent=p.stack.last().map(|x|x.0.as_str());
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
      if key=="id"{if p.ids.insert(value.to_string(),String::new()).is_some(){p.errors.push(format!("重複ID: {value}"))}id=Some(value.to_string());}
     }
     if tag=="body"{if p.body_seen{p.errors.push("bodyが重複".into())}p.body=true;p.body_seen=true;}
     if p.body&&tag=="section"{p.sections+=1}if p.body&&tag=="li"{p.items+=1}
     if !["meta","br","hr","img"].contains(&tag.as_str()){if t.self_closing{p.errors.push("非voidタグの自己終了".into())}p.stack.push((tag,id));}
    }else if t.kind==EndTag{
     if p.stack.pop().as_ref().map(|x|x.0.as_str())!=Some(tag.as_str()){p.errors.push(format!("閉じタグの不整合: {tag}"))}
     if tag=="body"{p.body=false;}
    }
   },
   Token::CharacterTokens(text)=>{
    if p.stack.last().is_some_and(|x|x.0=="style"){p.css.push_str(&text);}
    else if p.body{
     p.text.push_str(&text);p.text.push(' ');
     let ids=p.stack.iter().filter_map(|x|x.1.clone()).collect::<Vec<_>>();for id in ids{p.ids.get_mut(&id).unwrap().push_str(&text);}
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
 let lower=raw.to_ascii_lowercase();
 if raw.len()>2*1024*1024||!lower.trim_start().starts_with("<!doctype html>")||!lower.trim_end().ends_with("</html>"){return Err("完成HTMLの境界・サイズが不正です".into())}
 let input=BufferQueue::default();input.push_back(raw.into());let tok=Tokenizer::new(Sink(RefCell::new(Document::default()),images.to_vec()),TokenizerOpts::default());let _=tok.feed(&input);tok.end();
 let p=tok.sink.0.into_inner();
 if !p.errors.is_empty(){return Err(p.errors.join(" / "))}
 crate::css_policy::validate(&p.css,false)?;
 if !p.stack.is_empty(){return Err("整合した閉じタグが必要です".into())}
 for id in &p.refs{if p.ids.get(id).is_none_or(|text|text.trim().is_empty()){return Err(format!("未定義または空のARIA参照: {id}"))}}
 Ok(p)
}
pub fn require_structure(p:&Document)->Result<(),String>{
 if !p.ja||!p.utf8||!p.viewport||!p.title||!p.h1{return Err("日本語・UTF-8・viewport・title・本文h1が必要です".into())}
 if !p.body_seen||p.sections<2||p.items<3{return Err("body・2 section・3 liが必要です".into())}Ok(())
}
pub fn require_document_structure(p:&Document)->Result<(),String>{
 if !p.ja||!p.utf8||!p.viewport||!p.title||!p.h1||!p.body_seen||p.sections<1{return Err("日本語・UTF-8・viewport・title・本文h1・sectionが必要です".into())}Ok(())
}
