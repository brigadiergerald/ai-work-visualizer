//! Fail-closed static CSS policy. CSS Syntax tokens decode escapes before checks.
//! This is a restricted document dialect, not a browser or a general CSS sanitizer.
use cssparser::{Delimiter,ParseError,Parser,ParserInput,Token};
type E<'i> = ParseError<'i,String>;
#[derive(Debug)] enum Atom { Ident(String), Number(f32), Length(f32), Percent(f32), Hash(String), String, Punct(char), Function(String,Vec<Atom>) }
fn error<'i,T>(p:&Parser<'i,'_>,s:&str)->Result<T,E<'i>>{Err(p.new_custom_error(s.to_string()))}
fn atoms<'i>(p:&mut Parser<'i,'_>,depth:usize)->Result<Vec<Atom>,E<'i>>{
 if depth>12{return error(p,"CSS nesting limit")}
 let mut out=vec![];
 while !p.is_exhausted(){
  let a=match p.next()?.clone(){
   Token::Ident(s)=>Atom::Ident(s.to_ascii_lowercase()),
   Token::Number{value,..} if value.is_finite()=>Atom::Number(value),
   Token::Percentage{unit_value,..} if unit_value.is_finite()=>Atom::Percent(unit_value),
   Token::Dimension{value,unit,..} if value.is_finite()&&["px","rem","em","vw","vh","vmin","vmax","fr","cm","mm","in","pt","pc","ch","ex","deg"].contains(&unit.to_ascii_lowercase().as_str())=>Atom::Length(value),
   Token::Hash(s)|Token::IDHash(s)=>Atom::Hash(s.to_string()),
   Token::QuotedString(_)=>Atom::String,
   Token::Comma=>Atom::Punct(','),Token::Delim('/')=>Atom::Punct('/'),
   Token::Function(s) if ["rgb","rgba","hsl","hsla","clamp","min","max","linear-gradient","radial-gradient"].contains(&s.to_ascii_lowercase().as_str())=>{
    let v=p.parse_nested_block(|p|atoms(p,depth+1))?;Atom::Function(s.to_ascii_lowercase(),v)
   },
   _=>return error(p,"CSS URL, executable function, substitution or unsupported token"),
  };out.push(a);
 }
 if out.is_empty(){return error(p,"Empty CSS value")}
 Ok(out)
}
fn positive_size(a:&Atom)->bool{match a{Atom::Length(n)|Atom::Percent(n)=>*n>0.,Atom::Ident(s)=>["xx-small","x-small","small","medium","large","x-large","xx-large","smaller","larger"].contains(&s.as_str()),Atom::Function(s,a) if ["clamp","min","max"].contains(&s.as_str())=>a.iter().all(|x|matches!(x,Atom::Punct(','))||positive_size(x)),_=>false}}
fn visible_color(a:&Atom)->bool{match a{
 Atom::Ident(s)=>s!="transparent",
 Atom::Hash(s)=>!((s.len()==4&&s.ends_with('0'))||(s.len()==8&&s.ends_with("00"))),
 Atom::Function(s,v) if ["rgb","rgba","hsl","hsla"].contains(&s.as_str())=>{
  let nums=v.iter().filter_map(|x|match x{Atom::Number(n)|Atom::Percent(n)=>Some(*n),_=>None}).collect::<Vec<_>>();
  nums.len()==3||(nums.len()==4&&nums[3]>0.)
 },_=>false}}
fn declarations<'i>(p:&mut Parser<'i,'_>)->Result<(),E<'i>>{
 while !p.is_exhausted(){
  if p.try_parse(|p|p.expect_semicolon()).is_ok(){continue}
  let name=p.expect_ident_cloned()?.to_ascii_lowercase();p.expect_colon()?;
  // Finite properties exclude clipping, positioning, transforms, generated content,
  // animations, fonts loaded from elsewhere and arbitrary custom properties.
  if !["color","background","background-color","font-family","font-size","font-weight","font-style","line-height","letter-spacing","text-align","text-decoration","text-transform","display","visibility","opacity","box-sizing","margin","margin-top","margin-bottom","margin-left","margin-right","padding","padding-top","padding-bottom","padding-left","padding-right","border","border-top","border-bottom","border-left","border-right","border-color","border-width","border-style","border-radius","box-shadow","width","min-width","max-width","height","min-height","max-height","gap","row-gap","column-gap","grid-template-columns","grid-template-rows","align-items","justify-content","flex-direction","flex-wrap","flex","list-style","list-style-type","list-style-position","overflow-wrap","word-break","white-space","color-scheme","break-inside","break-before","break-after","page-break-inside","page-break-before","page-break-after","print-color-adjust"].contains(&name.as_str()){return error(p,&format!("Unsupported CSS property: {name}"))}
  let v=p.parse_until_before(Delimiter::Semicolon,|p|atoms(p,0))?;
  let single_ident=|allow:&[&str]|v.len()==1&&matches!(&v[0],Atom::Ident(s) if allow.contains(&s.as_str()));
  let allowed=match name.as_str(){
   "font-size"=>v.len()==1&&positive_size(&v[0]),
   "opacity"=>v.len()==1&&matches!(v[0],Atom::Number(n)|Atom::Percent(n) if n>0.&&n<=1.),
   "visibility"=>single_ident(&["visible"]),
   "display"=>single_ident(&["block","inline","inline-block","grid","inline-grid","flex","inline-flex","list-item","table","table-row","table-cell","flow-root"]),
   "color"=>v.len()==1&&visible_color(&v[0]),
   "width"|"height"|"max-width"|"max-height"=>!v.iter().any(|x|matches!(x,Atom::Number(n)|Atom::Length(n)|Atom::Percent(n) if *n<=0.)),
   "margin"|"margin-top"|"margin-bottom"|"margin-left"|"margin-right"|"padding"|"padding-top"|"padding-bottom"|"padding-left"|"padding-right"|"line-height"|"letter-spacing"=>!v.iter().any(|x|matches!(x,Atom::Number(n)|Atom::Length(n)|Atom::Percent(n) if *n<0.)),
   _=>true,
  };
  if !allowed{return error(p,&format!("Concealment or unsupported value: {name}"))}
 }
 Ok(())
}
fn media_feature<'i>(p:&mut Parser<'i,'_>)->Result<(),E<'i>>{
 let feature=p.expect_ident_cloned()?.to_ascii_lowercase();p.expect_colon()?;
 if !["width","min-width","max-width","height","min-height","max-height","orientation"].contains(&feature.as_str()){return error(p,"Unsupported media feature")}
 let v=atoms(p,0)?;
 if v.len()!=1||!matches!(&v[0],Atom::Length(n) if *n>0.)&&!matches!(&v[0],Atom::Ident(s) if feature=="orientation"&&["portrait","landscape"].contains(&s.as_str())){return error(p,"Invalid media feature")}
 Ok(())
}
fn rules<'i>(p:&mut Parser<'i,'_>,depth:usize)->Result<(),E<'i>>{
 if depth>8{return error(p,"CSS rule nesting limit")}
 while !p.is_exhausted(){
  let mut prelude=0;let mut media=false;
  loop{
   match p.next()?.clone(){
    Token::AtKeyword(s) if prelude==0&&s.eq_ignore_ascii_case("media")=>media=true,
    Token::Ident(s) if !media||["print","screen","all","and","or","not","only"].contains(&s.to_ascii_lowercase().as_str())=>{},
    Token::ParenthesisBlock if media=>p.parse_nested_block(media_feature)?,
    Token::CurlyBracketBlock if prelude>0=>{p.parse_nested_block(|p|if media{rules(p,depth+1)}else{declarations(p)})?;break},
    Token::IDHash(_)|Token::Hash(_)|Token::Colon|Token::Comma if !media=>{},
    Token::Delim(c) if !media&&['.','*','>','+','~','|'].contains(&c)=>{},
    _=>return error(p,"Unsupported selector, at-rule or malformed CSS"),
   };prelude+=1;
  }
 }
 Ok(())
}
pub fn validate(raw:&str,inline:bool)->Result<(),String>{
 // Unterminated CSS comments/strings/blocks must not be silently auto-closed.
 let mut input=ParserInput::new(raw);let mut parser=Parser::new(&mut input);
 let result=if inline{declarations(&mut parser)}else{rules(&mut parser,0)};
 result.map_err(|e|format!("CSS policy: {e:?}"))
}
