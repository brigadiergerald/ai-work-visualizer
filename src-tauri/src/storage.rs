use crate::model::*;
use rusqlite::{params,Connection,OptionalExtension};
use std::path::Path;
pub struct Store(pub Connection);
impl Store{
 pub fn open(path:&Path)->Result<Self,String>{
  let c=Connection::open(path).map_err(|e|e.to_string())?;
  c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS state(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS nodes(job_id TEXT,node_id TEXT,json TEXT NOT NULL,PRIMARY KEY(job_id,node_id)); CREATE TABLE IF NOT EXISTS events(id TEXT PRIMARY KEY,timestamp TEXT,job_id TEXT,node_id TEXT,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS approvals(id TEXT PRIMARY KEY,job_id TEXT,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS instructions(id TEXT PRIMARY KEY,job_id TEXT,kind TEXT,json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS artifacts(job_id TEXT,path TEXT,json TEXT NOT NULL,PRIMARY KEY(job_id,path)); CREATE TABLE IF NOT EXISTS purpose_job_context(job_id TEXT PRIMARY KEY REFERENCES jobs(id),version INTEGER NOT NULL,json TEXT NOT NULL);").map_err(|e|e.to_string())?;c.execute_batch(crate::presentation::SQL).map_err(|e|e.to_string())?;Ok(Self(c))
 }
 pub fn load(&self)->Result<Option<Snapshot>,String>{
  let raw:Option<String>=self.0.query_row("SELECT json FROM state WHERE id=1",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
  let s:Option<Snapshot>=raw.map(|v|serde_json::from_str(&v).map_err(|e|format!("保存データを読み込めません: {e}"))).transpose()?;
  if let Some(s)=&s{validate_contexts(&self.0,s,true)?;crate::presentation::validate(&self.0,s,true)?;}else{let count:i64=self.0.query_row("SELECT (SELECT count(*) FROM purpose_job_context)+(SELECT count(*) FROM task_execution_binding)+(SELECT count(*) FROM presentation_event_evidence)+(SELECT count(*) FROM events)",[],|r|r.get(0)).map_err(|e|e.to_string())?;if count!=0{return Err("追加証拠に対応する保存状態がありません".into())}}Ok(s)
 }
 pub fn save(&mut self,s:&Snapshot)->Result<(),String>{
  let tx=self.0.transaction().map_err(|e|e.to_string())?;
  validate_contexts(&tx,s,false)?;crate::presentation::validate(&tx,s,false)?;
  tx.execute("INSERT INTO state(id,json) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",[serde_json::to_string(s).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
  if let Some(j)=&s.job{tx.execute("INSERT OR REPLACE INTO jobs VALUES(?1,?2)",params![j.job_id,serde_json::to_string(j).unwrap()]).map_err(|e|e.to_string())?;for(n,r)in &j.nodes{tx.execute("INSERT OR REPLACE INTO nodes VALUES(?1,?2,?3)",params![j.job_id,n,serde_json::to_string(r).unwrap()]).map_err(|e|e.to_string())?;}for a in &j.artifacts{tx.execute("INSERT OR REPLACE INTO artifacts VALUES(?1,?2,?3)",params![j.job_id,a.path,serde_json::to_string(a).unwrap()]).map_err(|e|e.to_string())?;}}
  for e in &s.events{crate::presentation::immutable_event(&tx,e)?;tx.execute("INSERT OR IGNORE INTO events VALUES(?1,?2,?3,?4,?5)",params![e.event_id,e.timestamp,e.job_id,e.node_id,serde_json::to_string(e).unwrap()]).map_err(|e|e.to_string())?;}
  for a in &s.approvals{tx.execute("INSERT OR REPLACE INTO approvals VALUES(?1,?2,?3)",params![a.id,a.job_id,serde_json::to_string(a).unwrap()]).map_err(|e|e.to_string())?;}
  for i in &s.instructions{tx.execute("INSERT OR IGNORE INTO instructions VALUES(?1,?2,?3,?4)",params![i.id,i.job_id,i.kind,serde_json::to_string(i).unwrap()]).map_err(|e|e.to_string())?;}
  for (id,c) in &s.purpose_job_context{tx.execute("INSERT INTO purpose_job_context(job_id,version,json) VALUES(?1,?2,?3) ON CONFLICT(job_id) DO NOTHING",params![id,c.version,serde_json::to_string(c).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;}
  crate::presentation::save(&tx,s)?;
  tx.commit().map_err(|e|e.to_string())
 }
}
fn validate_contexts(db:&Connection,s:&Snapshot,loading:bool)->Result<(),String>{
 for (id,c) in &s.purpose_job_context{let j=s.job.iter().chain(s.history.iter()).find(|j|&j.job_id==id).ok_or("Purpose contextに対応するJobがありません")?;crate::purposes::validate_context(c,j)?;}
 let mut q=db.prepare("SELECT job_id,version,json FROM purpose_job_context").map_err(|e|e.to_string())?;
 let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?))).map_err(|e|e.to_string())?;let mut count=0;
 for row in rows{let (id,version,raw)=row.map_err(|e|e.to_string())?;let c:crate::purposes::JobContext=serde_json::from_str(&raw).map_err(|e|e.to_string())?;if s.purpose_job_context.get(&id)!=Some(&c)||c.version!=version{return Err("永続Purpose contextの競合・欠落です。上書きしません".into())}count+=1;}
 if loading&&count!=s.purpose_job_context.len(){return Err("Jobと永続Purpose contextの件数が一致しません".into())}Ok(())
}
#[cfg(test)] mod tests{use super::*;#[test]fn roundtrip(){let mut db=Store::open(Path::new(":memory:")).unwrap();let mut s=Snapshot::new(Path::new("C:/test"));s.event(None,"test","queued","日本語の記録",None,vec![]);db.save(&s).unwrap();let loaded=db.load().unwrap().unwrap();assert_eq!(loaded.events[0].message,"日本語の記録");assert_eq!(loaded.workflow.nodes.len(),5);}}

#[cfg(test)]
mod uint_compat_tests {
 use rusqlite::{params, Connection, Error};
 #[test]
 fn integer_storage_reopens_without_migration_and_rejects_out_of_range() {
  let path=crate::tests::test_root("uint-compat").join("revisions.sqlite");
  {
   let db=Connection::open(&path).unwrap();
   db.execute_batch("CREATE TABLE revisions(value INTEGER NOT NULL)").unwrap();
   // Simulate existing SQLite INTEGER rows, before the dependency update.
   for n in [0i64,1,i64::MAX] { db.execute("INSERT INTO revisions VALUES(?1)",[n]).unwrap(); }
  }
  {
   let mut db=Connection::open(&path).unwrap();
   let values=db.prepare("SELECT value FROM revisions ORDER BY rowid").unwrap()
    .query_map([],|r|r.get::<_,u64>(0)).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
   assert_eq!(values,vec![0,1,i64::MAX as u64]);
   for n in values {
    let (v,kind):(u64,String)=db.query_row("SELECT ?1,typeof(?1)",params![n],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!((v,kind),(n,"integer".into()));
   }
   assert!(matches!(db.query_row("SELECT -1",[],|r|r.get::<_,u64>(0)),Err(Error::IntegralValueOutOfRange(..))));
   for sql in ["SELECT 1.5","SELECT '1'","SELECT NULL"] {
    assert!(matches!(db.query_row(sql,[],|r|r.get::<_,u64>(0)),Err(Error::InvalidColumnType(..))));
   }
   for n in [i64::MAX as u64+1,u64::MAX] {
    let tx=db.transaction().unwrap();
    tx.execute("INSERT INTO revisions VALUES(7)",[]).unwrap();
    assert!(tx.execute("INSERT INTO revisions VALUES(?1)",params![n]).is_err());
    // A failed save must roll back earlier writes in the same transaction.
    drop(tx);
   }
   assert_eq!(db.query_row("SELECT count(*) FROM revisions",[],|r|r.get::<_,i64>(0)).unwrap(),3);
  }
  // Retain the database in the isolated test root, like the other storage tests.
 }
}
