/** Dev-only UI peer. Data comes from Rust's real offline runner, not a live agent. */
import fixture from '../../tests/fixtures/open-core.json';
import type {CoreSnapshot,CoreClient,Action} from './contract';
let snapshot=structuredClone(fixture.awaiting) as CoreSnapshot;
const listeners=new Set<(s:CoreSnapshot)=>void>();
export const fixtureClient:CoreClient={
 environment:'offline-fixture',
 async getSnapshot(){return snapshot;},
 async subscribe(fn){listeners.add(fn);return()=>{listeners.delete(fn);};},
 async execute<T>(a:Action){
  const job=snapshot.jobs.find(j=>'jobId' in a&&j.id===a.jobId);
  if(a.action==='readText'&&job&&job.artifacts.some(f=>f.path===a.path&&f.role==='product')){
   if(a.purpose==='copy'&&job.state!=='completed')throw new Error('OFFLINE: 承認前はコピーできません');
   return fixture.text as T;
  }
  if(a.action==='answer'&&job&&a.decision==='accept'&&job.approvals.some(p=>p.id===a.approvalId)){
   snapshot=structuredClone(fixture.completed) as CoreSnapshot;listeners.forEach(fn=>fn(snapshot));return null as T;
  }
  throw new Error('OFFLINE FIXTURE: 保存済みの文章確認・承認・コピーだけを検証します。実行・外部操作はしません。');
 }
};
