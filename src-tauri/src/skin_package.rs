//! Inert Skin Package v1. No extraction helper, code, URLs or execution hooks.
use std::{collections::{BTreeMap,BTreeSet},io::{Cursor,Read},time::Instant};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
pub const MAX_ZIP:usize=8*1024*1024;
pub const MAX_TOTAL:usize=32*1024*1024;
pub const MAX_FILE:usize=8*1024*1024;
pub const MAX_ENTRIES:usize=256;
pub const MAX_PIXELS:u64=16*1024*1024;
pub fn hash(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
pub fn err(code:&str)->String{format!("SKIN_{code}")}
pub fn safe_path(s:&str)->Result<(),String>{
 if s.is_empty()||s.len()>180||s.starts_with('/')||!s.bytes().all(|b|b.is_ascii_alphanumeric()||b"-._/".contains(&b)){return Err(err("PATH"))}
 for c in s.split('/'){
  let stem=c.split('.').next().unwrap_or("").to_ascii_uppercase();
  if c.is_empty()||c=="."||c==".."||c.ends_with('.')||c.len()>80||["CON","PRN","AUX","NUL","CLOCK$","CONIN$","CONOUT$"].contains(&stem.as_str())||((stem.starts_with("COM")||stem.starts_with("LPT"))&&stem.len()==4&&stem.as_bytes()[3].is_ascii_digit()){return Err(err("PATH"))}
 }Ok(())
}
pub fn id(s:&str)->bool{!s.is_empty()&&s.len()<=64&&s.as_bytes()[0].is_ascii_lowercase()&&s.bytes().all(|c|c.is_ascii_lowercase()||c.is_ascii_digit()||c==b'-')}
fn semver(s:&str)->Option<[u32;3]>{let p=s.split('.').map(|n|if n.is_empty()||n.len()>8||!n.bytes().all(|b|b.is_ascii_digit())||n.len()>1&&n.starts_with('0'){None}else{n.parse().ok()}).collect::<Option<Vec<u32>>>()?;p.try_into().ok()}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct FileSpec{pub path:String,pub bytes:usize,pub sha256:String}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Contracts{pub purpose:Vec<u32>,pub event_role:Vec<u32>,pub locale:Vec<u32>,pub skin:Vec<u32>}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct License{pub id:String,pub file:String,pub notices:String}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Bitmap{pub asset_id:String,pub path:String,pub width:u32,pub height:u32,pub frame_width:u32,pub frame_height:u32,pub columns:u32,pub rows:u32}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Parts{pub backgrounds:Vec<Bitmap>,pub sprites:Vec<Bitmap>,pub effects:Vec<Bitmap>}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Manifest{pub schema_version:u32,pub skin_id:String,pub version:String,pub min_core_version:String,pub supported_contracts:Contracts,pub display_name:BTreeMap<String,String>,pub description:BTreeMap<String,String>,pub locales:Vec<String>,pub license:License,pub files:Vec<FileSpec>,pub visual_parts:Parts}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Palette{pub room:String,pub floor:String,pub wood:String,pub wood_edge:String,pub mint:String}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Character{pub character_id:String,pub asset_id:String,pub prop:String}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Assignment{pub role_id:String,pub character_id:String}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Layout{pub schema_version:u32,pub palette:Palette,pub columns:u32,pub background_asset_id:Option<String>,pub characters:Vec<Character>,pub assignments:Vec<Assignment>}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Animation{pub role_id:String,pub status:String,pub asset_id:String,pub row:u32,pub frame:u32,pub frame_count:u32,pub duration_ms:u32,pub iterations:u32}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Animations{pub schema_version:u32,pub animations:Vec<Animation>}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Locale{pub schema_version:u32,pub locale:String,pub namespace:String,pub strings:BTreeMap<String,String>}
#[derive(Clone,Debug)]pub struct Package{pub manifest:Manifest,pub layout:Layout,pub animations:Animations,pub locale:BTreeMap<String,Locale>,pub files:BTreeMap<String,Vec<u8>>,pub sha256:String}
fn parse<T:serde::de::DeserializeOwned>(b:&[u8])->Result<T,String>{if b.len()>256*1024{return Err(err("JSON_SIZE"))}serde_json::from_slice(b).map_err(|_|err("SCHEMA"))}
fn text(s:&str,max:usize)->bool{!s.trim().is_empty()&&s.chars().count()<=max&&!s.chars().any(|c|c.is_control()||('\u{202a}'..='\u{202e}').contains(&c)||('\u{2066}'..='\u{2069}').contains(&c))}
fn localized(m:&BTreeMap<String,String>,max:usize)->bool{m.len()==2&&["ja","en"].iter().all(|k|m.get(*k).is_some_and(|s|text(s,max)))}
pub fn validate(bytes:&[u8])->Result<Package,String>{
 let start=Instant::now();if bytes.len()>MAX_ZIP||bytes.is_empty(){return Err(err("ARCHIVE_SIZE"))}
 let mut zip=zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_|err("ZIP"))?;
 if zip.len()==0||zip.len()>MAX_ENTRIES{return Err(err("ENTRY_LIMIT"))}if zip.has_duplicate_names(){return Err(err("DUPLICATE_PATH"))}
 if zip.has_overlapping_files().map_err(|_|err("ZIP"))?{return Err(err("OVERLAPPING_ENTRIES"))}
 let mut seen=BTreeSet::new();let mut dirs=BTreeSet::new();let mut declared=0usize;
 // Preflight ALL entries before reading any payload.
 for i in 0..zip.len(){
  let f=zip.by_index(i).map_err(|_|err("ZIP"))?;
  let name=std::str::from_utf8(f.name_raw()).map_err(|_|err("PATH"))?;
  let n=if f.is_dir(){name.strip_suffix('/').ok_or_else(||err("PATH"))?}else{name};safe_path(n)?;
  if !seen.insert(n.to_ascii_lowercase()){return Err(err("DUPLICATE_PATH"))}
  let kind=f.unix_mode().unwrap_or(0)&0o170000;
  if f.encrypted()||f.is_symlink()||f.external_attributes()&0x400!=0||!matches!(kind,0|0o100000|0o040000)||f.is_dir()&&kind==0o100000||!f.is_dir()&&kind==0o040000||f.extra_data_fields().next().is_some(){return Err(err("ZIP_ATTRIBUTES"))}
  if ![zip::CompressionMethod::Stored,zip::CompressionMethod::Deflated].contains(&f.compression()){return Err(err("ZIP_METHOD"))}
  if f.size()>MAX_FILE as u64||f.size()>f.compressed_size().max(1).saturating_mul(100){return Err(err("EXPANSION_LIMIT"))}
  declared=declared.checked_add(f.size() as usize).ok_or_else(||err("EXPANSION_LIMIT"))?;if declared>MAX_TOTAL{return Err(err("EXPANSION_LIMIT"))}
  if f.is_dir(){if f.size()!=0{return Err(err("ZIP_ATTRIBUTES"))}dirs.insert(n.to_string());}
 }
 let mut files=BTreeMap::new();let mut total=0usize;
 for i in 0..zip.len(){
  let mut f=zip.by_index(i).map_err(|_|err("ZIP"))?;if f.is_dir(){continue}
  let name=std::str::from_utf8(f.name_raw()).map_err(|_|err("PATH"))?.to_string();safe_path(&name)?;
  let expected=f.size();let ratio=f.compressed_size().max(1).saturating_mul(100);
  let mut data=Vec::new();let mut buf=[0u8;16384];
  loop{let n=f.read(&mut buf).map_err(|_|err("ZIP_DECODE"))?;if n==0{break}total+=n;
   if data.len()+n>MAX_FILE||total>MAX_TOTAL||(data.len()+n)as u64>ratio||start.elapsed().as_secs()>10{return Err(err("EXPANSION_LIMIT"))}data.extend_from_slice(&buf[..n]);
  }if data.len() as u64!=expected{return Err(err("ZIP_SIZE"))}files.insert(name,data);
 }
 if dirs.iter().any(|d|!files.keys().any(|p|p.starts_with(&(d.clone()+"/")))){return Err(err("UNREGISTERED_DIRECTORY"))}
 validate_files(files,hash(bytes))
}
pub fn validate_files(files:BTreeMap<String,Vec<u8>>,sha256:String)->Result<Package,String>{
 let get=|n:&str|files.get(n).ok_or_else(||err("MISSING_FILE"));
 let manifest:Manifest=parse(get("manifest.json")?)?;
 let m=&manifest;
 if m.schema_version!=1{return Err(err("SCHEMA_VERSION"))}
 if !id(&m.skin_id)||m.skin_id.starts_with("builtin-")||semver(&m.version).is_none(){return Err(err("IDENTITY"))}
 let min=semver(&m.min_core_version).ok_or_else(||err("CORE_VERSION"))?;if min>[0,1,0]{return Err(err("INCOMPATIBLE"))}
 if [&m.supported_contracts.purpose,&m.supported_contracts.event_role,&m.supported_contracts.locale,&m.supported_contracts.skin].iter().any(|v|v.is_empty()||v.len()>8||!v.contains(&1)){return Err(err("INCOMPATIBLE"))}
 if m.locales!=["ja","en"]||!localized(&m.display_name,80)||!localized(&m.description,320){return Err(err("LOCALE"))}
 if !text(&m.license.id,80)||m.license.file!="LICENSE"||m.license.notices!="NOTICES"{return Err(err("LICENSE"))}
 let mut listed=BTreeSet::new();for f in &m.files{safe_path(&f.path)?;if f.path=="manifest.json"||!listed.insert(f.path.clone())||f.bytes>MAX_FILE||f.sha256.len()!=64||!f.sha256.bytes().all(|b|b.is_ascii_hexdigit()&& !b.is_ascii_uppercase()){return Err(err("FILE_LIST"))}
  let b=get(&f.path)?;if b.len()!=f.bytes||hash(b)!=f.sha256{return Err(err("HASH"))}
 }
 if listed.len()+1!=files.len()||files.keys().any(|p|p!="manifest.json"&&!listed.contains(p)){return Err(err("FILE_SET"))}
 for n in ["layout.json","animations.json","locale/ja.json","locale/en.json","LICENSE","NOTICES"]{let b=get(n)?;if b.is_empty(){return Err(err("MISSING_FILE"))}}
 for n in ["LICENSE","NOTICES"]{if get(n)?.len()>64*1024||std::str::from_utf8(get(n)?).is_err(){return Err(err("LICENSE"))}}
 let mut allowed:BTreeSet<String>=["manifest.json","layout.json","animations.json","locale/ja.json","locale/en.json","LICENSE","NOTICES"].iter().map(|s|s.to_string()).collect();
 let mut assets=BTreeMap::new();let mut pixels=0u64;
 for (prefix,list) in [("assets/backgrounds/",&m.visual_parts.backgrounds),("assets/sprites/",&m.visual_parts.sprites),("assets/effects/",&m.visual_parts.effects)]{
  if list.len()>32{return Err(err("ASSET_LIMIT"))}
  for a in list{
   safe_path(&a.path)?;
   if !id(&a.asset_id)||assets.insert(a.asset_id.clone(),a).is_some()||!a.path.starts_with(prefix)||!a.path.ends_with(".png")||!allowed.insert(a.path.clone()){return Err(err("ASSET_REFERENCE"))}
   if a.width==0||a.height==0||a.width>4096||a.height>4096||a.frame_width==0||a.frame_height==0||a.columns==0||a.columns>8||a.rows==0||a.rows>8||a.frame_width.checked_mul(a.columns)!=Some(a.width)||a.frame_height.checked_mul(a.rows)!=Some(a.height){return Err(err("IMAGE_DIMENSIONS"))}
   if prefix=="assets/sprites/"&&(a.frame_width>96||a.frame_height>96||a.frame_width<16||a.frame_height<16){return Err(err("SPRITE_DIMENSIONS"))}
   pixels+=a.width as u64*a.height as u64;if pixels>MAX_PIXELS{return Err(err("PIXEL_LIMIT"))}
   let bytes=get(&a.path)?;
   let mut reader=image::ImageReader::with_format(Cursor::new(bytes),image::ImageFormat::Png);
   let mut limits=image::Limits::default();limits.max_image_width=Some(4096);limits.max_image_height=Some(4096);limits.max_alloc=Some(64*1024*1024);reader.limits(limits);
   let im=reader.decode().map_err(|_|err("IMAGE_DECODE"))?;
   if im.width()!=a.width||im.height()!=a.height{return Err(err("IMAGE_DIMENSIONS"))}
  }
 }
 if files.keys().any(|n|!allowed.contains(n)){return Err(err("FORBIDDEN_FILE"))}
 let layout:Layout=parse(get("layout.json")?)?;
 if layout.schema_version!=1||layout.columns>5||layout.characters.is_empty()||layout.characters.len()>32||layout.assignments.len()>64{return Err(err("LAYOUT"))}
 let color=|c:&str|c.len()==7&&c.starts_with('#')&&c[1..].bytes().all(|b|b.is_ascii_hexdigit());
 let p=&layout.palette;if [&p.room,&p.floor,&p.wood,&p.wood_edge,&p.mint].iter().any(|s|!color(s)){return Err(err("PALETTE"))}
 if let Some(bg)=&layout.background_asset_id{if !m.visual_parts.backgrounds.iter().any(|a|&a.asset_id==bg){return Err(err("LAYOUT_REFERENCE"))}}
 let mut chars=BTreeSet::new();for c in &layout.characters{if !id(&c.character_id)||!chars.insert(&c.character_id)||!["paper","books","check","lamp","cabinet"].contains(&c.prop.as_str())||!m.visual_parts.sprites.iter().any(|a|a.asset_id==c.asset_id){return Err(err("LAYOUT_REFERENCE"))}}
 let mut roles=BTreeSet::new();for a in &layout.assignments{if !id(&a.role_id)||!roles.insert(&a.role_id)||!chars.contains(&a.character_id){return Err(err("LAYOUT_REFERENCE"))}}
 let animations:Animations=parse(get("animations.json")?)?;if animations.schema_version!=1||animations.animations.len()>384{return Err(err("ANIMATION"))}
 let mut pairs=BTreeSet::new();for a in &animations.animations{
  let bitmap=assets.get(&a.asset_id).ok_or_else(||err("ANIMATION_REFERENCE"))?;
  let look=layout.assignments.iter().find(|v|v.role_id==a.role_id).and_then(|v|layout.characters.iter().find(|c|c.character_id==v.character_id));
  if look.is_none_or(|c|c.asset_id!=a.asset_id)||!pairs.insert((&a.role_id,&a.status))||!["IDLE","RUNNING","AWAITING_APPROVAL","SUCCEEDED","FAILED","UNKNOWN"].contains(&a.status.as_str())||a.row>=bitmap.rows||a.frame_count==0||a.frame.checked_add(a.frame_count).is_none_or(|n|n>bitmap.columns)||a.duration_ms<100||a.duration_ms>3000||a.iterations==0||a.iterations>8||a.status!="RUNNING"&&a.frame_count!=1{return Err(err("ANIMATION"))}
 }
 let mut locale=BTreeMap::new();for l in ["ja","en"]{
  let cat:Locale=parse(get(&format!("locale/{l}.json"))?)?;
  if cat.schema_version!=1||cat.locale!=l||cat.namespace!="skin"||cat.strings.len()>128||cat.strings.iter().any(|(k,v)|!k.starts_with("skin.")||k.len()>120||!k.bytes().all(|b|b.is_ascii_alphanumeric()||b".-_".contains(&b))||!text(v,320))||cat.strings.get("skin.name")!=m.display_name.get(l)||cat.strings.get("skin.description")!=m.description.get(l){return Err(err("LOCALE_NAMESPACE"))}locale.insert(l.to_string(),cat);
 }
 if locale["ja"].strings.keys().ne(locale["en"].strings.keys()){return Err(err("LOCALE_KEYS"))}
 Ok(Package{manifest,layout,animations,locale,files,sha256})
}
