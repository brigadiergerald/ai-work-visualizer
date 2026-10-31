//! A presentation-only library. Does not receive Engine, CoreClient or Job state.
use std::{fs::{self,OpenOptions},io::{Read,Write},path::{Path,PathBuf},sync::Mutex,collections::BTreeMap};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use base64::Engine as _;
use crate::skin_package::{self as pkg,Package};
pub struct Library{root:PathBuf,gate:Mutex<()>}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Selection{pub skin_id:String,pub version:String,pub package_sha256:String}
#[derive(Deserialize)]#[serde(tag="operation",rename_all="camelCase",deny_unknown_fields)]
pub enum Request{Get{#[serde(rename="legacyTheme")]legacy_theme:Option<String>},Import{bytes:Vec<u8>},Select{selection:Selection},Fallback{selection:Selection,reason:String}}
#[derive(Serialize)]#[serde(rename_all="camelCase")]
pub struct Entry{selection:Selection,display_name:BTreeMap<String,String>,description:BTreeMap<String,String>,state:String,builtin:bool,reason:Option<String>}
fn builtin(mint:bool)->Selection{Selection{skin_id:if mint{"builtin-mint"}else{"builtin-paper"}.into(),version:"1.0.0".into(),package_sha256:if mint{"builtin:free-mint-test:v1"}else{"builtin:free-standard:v1"}.into()}}
fn builtin_entry(mint:bool)->Entry{Entry{selection:builtin(mint),display_name:[("ja".into(),if mint{"Mint"}else{"Paper"}.into()),("en".into(),if mint{"Mint"}else{"Paper"}.into())].into(),description:[("ja".into(),"いつでも使える、小さな作業工房。".into()),("en".into(),"The built-in Small Creative Workshop.".into())].into(),state:"AVAILABLE".into(),builtin:true,reason:None}}
fn check_selection(s:&Selection)->Result<(),String>{if s==&builtin(false)||s==&builtin(true){return Ok(())}if !pkg::id(&s.skin_id)||s.skin_id.starts_with("builtin-")||s.version.split('.').count()!=3||!s.version.bytes().all(|b|b.is_ascii_digit()||b==b'.')||s.version.len()>26||s.package_sha256.len()!=64||!s.package_sha256.bytes().all(|b|b.is_ascii_hexdigit()){return Err(pkg::err("SELECTION"))}Ok(())}
fn plain_meta(path:&Path,dir:bool)->Result<std::fs::Metadata,String>{let m=fs::symlink_metadata(path).map_err(|_|pkg::err("MISSING"))?;if m.file_type().is_symlink()||m.is_dir()!=dir||!dir&&!m.is_file(){return Err(pkg::err("FILESYSTEM_LINK"))}#[cfg(windows)]{use std::os::windows::fs::MetadataExt;if m.file_attributes()&0x400!=0{return Err(pkg::err("FILESYSTEM_LINK"))}}#[cfg(unix)]{use std::os::unix::fs::MetadataExt;if !dir&&m.nlink()!=1{return Err(pkg::err("FILESYSTEM_LINK"))}}Ok(m)}
fn read_plain(path:&Path,max:usize)->Result<Vec<u8>,String>{
 let m=plain_meta(path,false)?;if m.len()>max as u64{return Err(pkg::err("FILE_SIZE"))}
 let mut options=OpenOptions::new();options.read(true);
 #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.custom_flags(0x00200000);}
 let f=options.open(path).map_err(|_|pkg::err("FILE_READ"))?;
 #[cfg(windows)]{
  use std::os::windows::io::AsRawHandle;
  #[repr(C)]struct Info{attrs:u32,creation:[u32;2],access:[u32;2],write:[u32;2],volume:u32,high:u32,low:u32,links:u32,index_high:u32,index_low:u32}
  #[link(name="kernel32")]unsafe extern "system"{fn GetFileInformationByHandle(h:*mut std::ffi::c_void,info:*mut Info)->i32;}
  let mut info:Info=unsafe{std::mem::zeroed()};if unsafe{GetFileInformationByHandle(f.as_raw_handle(),&mut info)}==0||info.links!=1||info.attrs&0x400!=0{return Err(pkg::err("FILESYSTEM_LINK"))}
 }
 let mut b=Vec::new();f.take(max as u64+1).read_to_end(&mut b).map_err(|_|pkg::err("FILE_READ"))?;if b.len()>max{return Err(pkg::err("FILE_SIZE"))}Ok(b)
}
fn make_dir(path:&Path)->Result<(),String>{if !path.exists(){fs::create_dir(path).map_err(|_|pkg::err("DIRECTORY_WRITE"))?}plain_meta(path,true)?;Ok(())}
fn write_new(p:&Path,b:&[u8])->Result<(),String>{let mut f=OpenOptions::new().create_new(true).write(true).open(p).map_err(|_|pkg::err("FILE_WRITE"))?;f.write_all(b).and_then(|_|f.sync_all()).map_err(|_|pkg::err("FILE_WRITE"))}
fn atomic_json(p:&Path,v:&Value)->Result<(),String>{if p.exists(){plain_meta(p,false)?;}let tmp=p.with_file_name(format!(".prefs-{}.tmp",uuid::Uuid::new_v4()));write_new(&tmp,&serde_json::to_vec_pretty(v).map_err(|_|pkg::err("PREFERENCES"))?)?;let result=fs::rename(&tmp,p).map_err(|_|pkg::err("PREFERENCES_WRITE"));if result.is_err(){let _=fs::remove_file(tmp);}result}
impl Library{
 pub fn new(profile:&Path)->Self{Self{root:profile.to_path_buf(),gate:Mutex::new(())}}
 fn paths(&self)->Result<(PathBuf,PathBuf),String>{plain_meta(&self.root,true)?;let skins=self.root.join("skins");let prefs=self.root.join("presentation");make_dir(&skins)?;make_dir(&prefs)?;Ok((skins,prefs))}
 fn audit(&self,kind:&str,selection:Option<&Selection>,reason:&str)->Result<(),String>{let (_,p)=self.paths()?;let path=p.join("skin-incidents.jsonl");if path.exists(){read_plain(&path,4*1024*1024)?;}let mut f=OpenOptions::new().create(true).append(true).open(path).map_err(|_|pkg::err("AUDIT_WRITE"))?;writeln!(f,"{}",json!({"at":chrono::Utc::now().to_rfc3339(),"kind":kind,"originalSelection":selection,"reason":reason})).and_then(|_|f.sync_all()).map_err(|_|pkg::err("AUDIT_WRITE"))}
 fn save(&self,s:&Selection)->Result<(),String>{check_selection(s)?;let (_,p)=self.paths()?;atomic_json(&p.join("skin-selection-v1.json"),&json!({"schemaVersion":1,"selectedSkin":s}))}
 fn load(&self,legacy:Option<&str>)->Result<(Selection,Option<String>),String>{
  let (_,p)=self.paths()?;let f=p.join("skin-selection-v1.json");
  if !f.exists(){let s=builtin(legacy==Some("free-mint-test"));self.save(&s)?;return Ok((s,None))}
  let result=read_plain(&f,65536).and_then(|b|{let v:Value=serde_json::from_slice(&b).map_err(|_|pkg::err("PREFERENCES"))?;if v["schemaVersion"]!=1{return Err(pkg::err("PREFERENCES"))}let s:Selection=serde_json::from_value(v["selectedSkin"].clone()).map_err(|_|pkg::err("PREFERENCES"))?;check_selection(&s)?;Ok(s)});
  match result{Ok(s)=>Ok((s,None)),Err(e)=>{self.audit("PREFERENCES_INVALID",None,&e)?;self.save(&builtin(false))?;Ok((builtin(false),Some(e)))}}
 }
 fn folder(&self,s:&Selection)->Result<PathBuf,String>{check_selection(s)?;let (base,_)=self.paths()?;let id=base.join(&s.skin_id);plain_meta(&id,true)?;let dir=id.join(&s.version);plain_meta(&dir,true)?;Ok(dir)}
 fn verify(&self,s:&Selection)->Result<Package,String>{
  let folder=self.folder(s)?;let bytes=read_plain(&folder.join("package.zip"),pkg::MAX_ZIP)?;
  if pkg::hash(&bytes)!=s.package_sha256{return Err(pkg::err("PACKAGE_HASH"))}
  let package=pkg::validate(&bytes)?;
  if package.manifest.skin_id!=s.skin_id||package.manifest.version!=s.version{return Err(pkg::err("IDENTITY"))}
  let content=folder.join("files");plain_meta(&content,true)?;
  fn enumerate(dir:&Path,prefix:&str,found:&mut BTreeMap<String,Vec<u8>>,total:&mut usize)->Result<(),String>{
   for entry in fs::read_dir(dir).map_err(|_|pkg::err("MISSING"))?{let entry=entry.map_err(|_|pkg::err("FILE_READ"))?;let name=entry.file_name().to_str().ok_or_else(||pkg::err("PATH"))?.to_string();let key=format!("{prefix}{name}");pkg::safe_path(&key)?;
    let m=fs::symlink_metadata(entry.path()).map_err(|_|pkg::err("FILE_READ"))?;
    if m.is_dir(){plain_meta(&entry.path(),true)?;if key.split('/').count()>6{return Err(pkg::err("PATH"))}enumerate(&entry.path(),&(key+"/"),found,total)?;}
    else{if found.len()>=pkg::MAX_ENTRIES{return Err(pkg::err("ENTRY_LIMIT"))}let bytes=read_plain(&entry.path(),pkg::MAX_FILE)?;*total+=bytes.len();if *total>pkg::MAX_TOTAL{return Err(pkg::err("EXPANSION_LIMIT"))}found.insert(key,bytes);}
   }Ok(())
  }
  let mut installed=BTreeMap::new();enumerate(&content,"",&mut installed,&mut 0)?;
  if installed!=package.files{return Err(pkg::err("INSTALLED_HASH"))}Ok(package)
 }
 fn install(&self,bytes:&[u8])->Result<Selection,String>{
  let p=pkg::validate(bytes)?;let s=Selection{skin_id:p.manifest.skin_id.clone(),version:p.manifest.version.clone(),package_sha256:p.sha256.clone()};
  let current=self.view(None,None)?;if current["entries"].as_array().is_some_and(|v|v.len()>=34)&&!current["entries"].as_array().unwrap().iter().any(|v|v["selection"]["skinId"]==s.skin_id&&v["selection"]["version"]==s.version){return Err(pkg::err("LIBRARY_LIMIT"))}let (base,_)=self.paths()?;let parent=base.join(&s.skin_id);make_dir(&parent)?;let dest=parent.join(&s.version);
  if dest.exists(){if self.verify(&s).is_ok(){return Ok(s)}return Err(pkg::err("VERSION_CONFLICT"))}
  let stage=base.join(format!(".staging-{}",uuid::Uuid::new_v4()));fs::create_dir(&stage).map_err(|_|pkg::err("STAGING"))?;
  let result=(||{
   write_new(&stage.join("package.zip"),bytes)?;let content=stage.join("files");make_dir(&content)?;
   for (name,data) in &p.files{pkg::safe_path(name)?;let target=content.join(name);let rel=target.parent().unwrap().strip_prefix(&content).map_err(|_|pkg::err("PATH"))?;let mut cursor=content.clone();
    for c in rel.components(){cursor.push(c);make_dir(&cursor)?;}plain_meta(&stage,true)?;plain_meta(target.parent().unwrap(),true)?;
    if !target.parent().unwrap().canonicalize().map_err(|_|pkg::err("PATH"))?.starts_with(content.canonicalize().map_err(|_|pkg::err("PATH"))?){return Err(pkg::err("PATH"))}write_new(&target,data)?;
   }
   // A crash leaves only an ignored staging directory. No catalog registration precedes rename.
   plain_meta(&base,true)?;plain_meta(&parent,true)?;if dest.exists(){return Err(pkg::err("VERSION_CONFLICT"))}
   fs::rename(&stage,&dest).map_err(|_|pkg::err("ATOMIC_INSTALL"))?;self.verify(&s)?;Ok(s.clone())
  })();
  // Never recursively remove a computed package path; failed staging is ignored and retained for diagnosis.
  if let Err(e)=&result{let _=self.audit("INSTALL_FAILED",Some(&s),e);}result
 }
 fn view(&self,legacy:Option<&str>,message:Option<&str>)->Result<Value,String>{
  let (base,_)=self.paths()?;let (mut selected,mut fallback)=self.load(legacy)?;let mut entries=vec![builtin_entry(false),builtin_entry(true)];
  let mut render_package=None;let mut count=0;
  for item in fs::read_dir(&base).map_err(|_|pkg::err("LIBRARY"))?.take(65){let item=item.map_err(|_|pkg::err("LIBRARY"))?;let id=item.file_name().to_string_lossy().into_owned();if id.starts_with('.')||!pkg::id(&id){continue}plain_meta(&item.path(),true)?;
   for version in fs::read_dir(item.path()).map_err(|_|pkg::err("LIBRARY"))?.take(33){let version=version.map_err(|_|pkg::err("LIBRARY"))?;plain_meta(&version.path(),true)?;
    count+=1;if count>32{return Err(pkg::err("LIBRARY_LIMIT"))}let version_name=version.file_name().to_string_lossy().into_owned();let original=read_plain(&version.path().join("package.zip"),pkg::MAX_ZIP);
    let s=Selection{skin_id:id.clone(),version:version_name,package_sha256:original.as_ref().map(|b|pkg::hash(b)).unwrap_or_default()};
    match self.verify(&s){Ok(p)=>{entries.push(Entry{selection:s.clone(),display_name:p.manifest.display_name.clone(),description:p.manifest.description.clone(),state:"AVAILABLE".into(),builtin:false,reason:None});if s==selected{render_package=Some(p);}},Err(reason)=>entries.push(Entry{display_name:[("ja".into(),id.clone()),("en".into(),id.clone())].into(),description:BTreeMap::new(),selection:s,state:if reason.contains("INCOMPATIBLE"){"INCOMPATIBLE"}else{"CORRUPT"}.into(),builtin:false,reason:Some(reason)})}
   }
  }
  if selected!=builtin(false)&&selected!=builtin(true)&&render_package.is_none(){
   let reason=self.verify(&selected).err().unwrap_or_else(||pkg::err("MISSING"));self.audit("FALLBACK",Some(&selected),&reason)?;self.save(&builtin(false))?;selected=builtin(false);fallback=Some(reason);
  }
  let render=render_package.as_ref().map(|p|{
   let assets=p.manifest.visual_parts.backgrounds.iter().chain(&p.manifest.visual_parts.sprites).chain(&p.manifest.visual_parts.effects).map(|a|{
    (a.asset_id.clone(),json!({"spec":a,"dataUrl":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(&p.files[&a.path]))}))
   }).collect::<BTreeMap<_,_>>();json!({"selection":selected,"layout":p.layout,"animations":p.animations,"locales":p.locale,"assets":assets})
  });
  Ok(json!({"contractVersion":1,"entries":entries,"selected":selected,"render":render,"fallbackReason":fallback,"message":message}))
 }
 pub fn perform(&self,r:Request)->Result<Value,String>{let _lock=self.gate.lock().map_err(|_|pkg::err("LOCK"))?;
  match r{
   Request::Get{legacy_theme}=>self.view(legacy_theme.as_deref(),None),
   Request::Import{bytes}=>{let s=match self.install(&bytes){Ok(s)=>s,Err(e)=>{let _=self.audit("REJECTED",None,&e);return Err(e)}};let mut v=self.view(None,Some("INSTALLED"))?;v["installed"]=json!(s);Ok(v)},
   Request::Select{selection}=>{check_selection(&selection)?;if selection!=builtin(false)&&selection!=builtin(true){self.verify(&selection)?;}self.save(&selection)?;self.view(None,Some("SELECTED"))},
   Request::Fallback{selection,reason}=>{let (current,_)=self.load(None)?;if current==selection{self.audit("RENDERER_FALLBACK",Some(&selection),&reason.chars().take(160).collect::<String>())?;self.save(&builtin(false))?;}self.view(None,Some("FALLBACK"))}
  }
 }
}
