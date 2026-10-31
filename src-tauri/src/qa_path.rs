use std::{ffi::OsString,path::PathBuf};

// Explicit, marked test directory only. Never moves or copies normal user data.
pub fn select_data_path(normal:PathBuf, requested:Option<OsString>)->Result<PathBuf,String>{
 let Some(requested)=requested else{return Ok(normal)};
 let path=PathBuf::from(requested);
 if !path.is_absolute(){return Err("QA保存先は絶対パスが必要です".into())}
 let path=path.canonicalize().map_err(|_|"QA保存先には既存のフォルダが必要です")?;
 if !path.is_dir()||!path.join(".ai-guild-qa").is_file(){return Err("QA専用フォルダのマーカーがありません".into())}
 if normal.canonicalize().ok().as_ref()==Some(&path){return Err("通常の保存先はQA保存先に指定できません".into())}
 Ok(path)
}

#[cfg(test)] mod tests{
 use super::*;
 #[test]fn normal_data_unchanged_and_qa_requires_explicit_isolation(){
  let root=std::env::var_os("AI_GUILD_TEST_ROOT").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("target/test-data")).join(format!("ai-guild-path-{}",crate::model::id()));
  std::fs::create_dir_all(&root).unwrap();std::fs::write(root.join(".ai-guild-qa"),"isolated path test").unwrap();
  let normal=root.join("normal");let qa=root.join("qa");std::fs::create_dir_all(&normal).unwrap();std::fs::create_dir_all(&qa).unwrap();
  assert_eq!(select_data_path(normal.clone(),None).unwrap(),normal);
  assert!(select_data_path(normal.clone(),Some("relative".into())).is_err());
  assert!(select_data_path(normal.clone(),Some(qa.clone().into_os_string())).is_err());
  std::fs::write(qa.join(".ai-guild-qa"),"local UI verification").unwrap();
  assert_eq!(select_data_path(normal.clone(),Some(qa.clone().into_os_string())).unwrap(),qa.canonicalize().unwrap());
  std::fs::write(normal.join(".ai-guild-qa"),"not allowed").unwrap();
  assert!(select_data_path(normal.clone(),Some(normal.into_os_string())).is_err());
 }
}
