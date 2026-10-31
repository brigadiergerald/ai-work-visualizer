//! OS open is distinct from browser automation and from a display-quality PASS.
use crate::{engine::{Engine,verify_artifact},model::*};
use std::{fs,path::{Path,PathBuf}};
fn regular_path(path:&Path)->PathBuf{
 #[cfg(windows)]{let s=path.to_string_lossy();if let Some(rest)=s.strip_prefix("\\\\?\\UNC\\"){return PathBuf::from(format!("\\\\{rest}"))}if let Some(rest)=s.strip_prefix("\\\\?\\"){return PathBuf::from(rest)}}
 path.to_path_buf()
}
pub fn prepare(s:&Snapshot,p:&str)->Result<(Job,Artifact,PathBuf),String>{
 let j=s.job.iter().chain(s.history.iter()).find(|j|j.artifacts.iter().any(|a|a.path==p)).ok_or("登録済み成果物だけを開けます")?;
 let a=j.artifacts.iter().find(|a|a.path==p).unwrap();verify_artifact(a,Path::new(&j.workspace))?;
 // Validate the original lexical route before canonicalizing; reject reparse hops.
 let path=regular_path(Path::new(p));let root=regular_path(Path::new(&j.workspace));
 if !path.is_absolute(){return Err("成果物は絶対パスが必要です".into())}
 let relative=path.strip_prefix(&root).map_err(|_|"ジョブの保存範囲外の成果物です")?;
 let mut cursor=root.to_path_buf();let mut hops=vec![cursor.clone()];
 for c in relative.components(){if !matches!(c,std::path::Component::Normal(_)){return Err("不正な成果物参照です".into())}cursor.push(c);hops.push(cursor.clone());}
 for hop in hops{let m=fs::symlink_metadata(hop).map_err(|e|e.to_string())?;
  #[cfg(windows)]{use std::os::windows::fs::MetadataExt;if m.file_attributes()&0x400!=0{return Err("reparse point経由の成果物は開けません".into())}}
  #[cfg(not(windows))]if m.file_type().is_symlink(){return Err("symlink経由の成果物は開けません".into())}
 }
 let resolved=path.canonicalize().map_err(|e|e.to_string())?;if !resolved.is_file(){return Err("通常ファイルではありません".into())}
 let ext=resolved.extension().and_then(|x|x.to_str()).unwrap_or("").to_ascii_lowercase();
 if !["md","txt","json","png","jpg","jpeg","pdf","csv","tsv","html","py","js","mjs","ts","tsx","rs","css"].contains(&ext.as_str()){return Err("実行可能ファイルは開きません".into())}
 if crate::general::applies(j)&&a.kind=="product"{let q=crate::general::verify_product(j)?;if j.quality.as_ref().is_none_or(|saved|saved.machine_status!="PASS"||saved.artifact_sha256!=q.artifact_sha256){return Err("納品bundleと品質検査が一致しません".into())}}
 if (ext=="html"&&!crate::general::applies(j))||(j.workflow_id=="codex-document-v1"&&a.node_id=="writing"){
  let q=crate::practical::verify_product(j)?;
  if j.quality.as_ref().is_none_or(|saved|saved.machine_status!="PASS"||saved.artifact_sha256!=a.sha256||q.artifact_sha256!=a.sha256){return Err("完成品QAとhashが一致する文書だけを開けます".into())}
 }
 Ok((j.clone(),a.clone(),regular_path(&resolved)))
}
impl Engine{
 pub fn open_artifact(&self,p:String)->Result<(),String>{
  let (j,a,path)=prepare(&self.snapshot(),&p)?;
  let log=|typ:&str,message:String|self.update(|s|{s.events.push(Event{event_id:id(),timestamp:now(),job_id:j.job_id.clone(),workflow_id:j.workflow_id.clone(),node_id:Some(a.node_id.clone()),event_type:typ.into(),status:j.status.clone(),message,artifact_paths:vec![a.path.clone()],edge_id:None,error_code:if typ=="artifact_open_failed"{Some(typ.into())}else{None},mode:j.mode.clone()});Ok(())});
  log("artifact_open_requested",format!("Windowsファイルオープン要求：{} / SHA-256 {}。表示成功・CUA成功の判定ではありません。",a.name,a.sha256))?;
  #[cfg(windows)]let result=if path.extension().and_then(|v|v.to_str()).is_some_and(|v|["py","js","mjs","ts","tsx","rs","css"].contains(&v.to_ascii_lowercase().as_str())){crate::rpc::command("notepad.exe").arg(&path).spawn().map(|_|()).map_err(|e|format!("ソースの閲覧を開始できません: {e}"))}else{use std::os::windows::ffi::OsStrExt;
   let file:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();let operation:Vec<u16>="open".encode_utf16().chain(Some(0)).collect();
   let code=unsafe{windows_sys::Win32::UI::Shell::ShellExecuteW(std::ptr::null_mut(),operation.as_ptr(),file.as_ptr(),std::ptr::null(),std::ptr::null(),1)} as isize;
   if code<=32{Err(format!("関連付けされたアプリで開けませんでした（Windows code {code}）"))}else{Ok(())}
  };
  #[cfg(not(windows))]let result:Result<(),String>={let _=path;Err("Windows以外のファイルオープンは未対応です".into())};
  match result{Ok(())=>log("artifact_open_os_accepted","Windowsがファイルオープン要求を受理しました。表示内容は別途確認してください。".into()),Err(e)=>{log("artifact_open_failed",e.clone())?;Err(e)}}
 }
}
