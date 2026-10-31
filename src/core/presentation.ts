import type {CoreClient} from './contract';
import type {PurposeJobView} from './purposes';

export type PresentationStatus='IDLE'|'RUNNING'|'AWAITING_APPROVAL'|'HANDOFF'|'SUCCEEDED'|'FAILED'|'UNKNOWN';
export type LiveEvidence='CURRENT'|'RESYNC_REQUIRED'|'UNKNOWN';
export interface RoleDefinition {readonly id:string;readonly purposeId:string;readonly labelKey:string;readonly actor:'codex'|'host'|'human'}
export interface PresentationTask {readonly taskId:string;readonly nodeId:string;readonly attempt:number;readonly roleId:string|null;readonly status:PresentationStatus;readonly legacyStatus:string|null;readonly reasonCode:string|null}
export interface PresentationEvent {
 readonly contractVersion:1;readonly eventId:string;readonly sourceEventId:string;readonly jobId:string;readonly taskId:string|null;
 readonly purposeId:string|null;readonly phase:string;readonly status:PresentationStatus;readonly roleId:string|null;readonly timestamp:string;
 readonly artifactIds:readonly string[];readonly approvalId:string|null;readonly sequence:number;readonly scope:'JOB'|'TASK'|'HANDOFF';
 readonly mode:'real'|'demo'|'local_only'|'system';readonly legacyStatus:string;readonly reasonCode:string|null;
 readonly handoff:{readonly edgeId:string;readonly fromTaskId:string;readonly toTaskId:string;readonly sourceEventId:string;readonly verification:'FILES_VERIFIED'}|null;
}
export interface PresentationSnapshot {
 readonly contractVersion:1;readonly jobId:string;readonly revision:number;readonly eventCursor:number;readonly events:readonly PresentationEvent[];
 readonly liveEvidence:LiveEvidence;readonly liveReasonCode:string|null;readonly purpose:PurposeJobView;readonly roleMapVersion:string|null;
 readonly roles:readonly RoleDefinition[];readonly tasks:readonly PresentationTask[];readonly jobStatus:PresentationStatus;readonly legacyJobStatus:string;
 readonly artifacts:readonly {readonly artifactId:string;readonly name:string;readonly sha256:string;readonly bytes:number;readonly integrity:'verified'|'failed'}[];
}
export interface PresentationNotice {
 readonly contractVersion:1;readonly previousRevision:number;readonly previousCursor:number;readonly revision:number;readonly eventCursor:number;
 readonly snapshots:readonly PresentationSnapshot[];
}
const integer=(n:number)=>Number.isSafeInteger(n)&&n>=0;
export function validatePresentation(s:PresentationSnapshot){
 if(s.contractVersion!==1||!s.jobId||!integer(s.revision)||!integer(s.eventCursor)||!['CURRENT','RESYNC_REQUIRED','UNKNOWN'].includes(s.liveEvidence)||!['IDLE','RUNNING','AWAITING_APPROVAL','SUCCEEDED','FAILED','UNKNOWN'].includes(s.jobStatus))throw Error('PRESENTATION_FRAME_INVALID');
 const ids=new Set<string>(),tasks=new Map(s.tasks.map(t=>[t.taskId,t])),roles=new Set(s.roles.map(r=>r.id)),artifacts=new Set(s.artifacts.map(a=>a.artifactId));let sequence=0;
 if(tasks.size!==s.tasks.length||roles.size!==s.roles.length||artifacts.size!==s.artifacts.length)throw Error('PRESENTATION_DUPLICATE_ID');
 for(const t of s.tasks)if(!t.taskId||!integer(t.attempt)||t.attempt===0||t.roleId!==null&&!roles.has(t.roleId)||!['IDLE','RUNNING','AWAITING_APPROVAL','SUCCEEDED','FAILED','UNKNOWN'].includes(t.status))throw Error('PRESENTATION_TASK_INVALID');
 for(const e of s.events){
  if(e.contractVersion!==1||!e.eventId||ids.has(e.eventId)||e.sourceEventId!==e.eventId||e.jobId!==s.jobId||!integer(e.sequence)||e.sequence<=sequence||e.sequence>s.eventCursor||!['IDLE','RUNNING','AWAITING_APPROVAL','HANDOFF','SUCCEEDED','FAILED','UNKNOWN'].includes(e.status)||!['JOB','TASK','HANDOFF'].includes(e.scope)||!['real','demo','local_only','system'].includes(e.mode))throw Error('PRESENTATION_EVENT_INVALID');
  if(e.purposeId!==s.purpose.purposeId||e.artifactIds.some(id=>!artifacts.has(id)))throw Error('PRESENTATION_CONTEXT_MISMATCH');
  if(e.taskId!==null&&(!tasks.has(e.taskId)||tasks.get(e.taskId)?.nodeId!==e.phase)||e.roleId!==null&&(!roles.has(e.roleId)||tasks.get(e.taskId??'')?.roleId!==e.roleId))throw Error('PRESENTATION_TASK_MISMATCH');
  if(e.status==='AWAITING_APPROVAL'&&!e.approvalId)throw Error('PRESENTATION_APPROVAL_MISSING');
  if(e.status==='HANDOFF'){
   const h=e.handoff;
   if(e.scope!=='HANDOFF'||!h||h.verification!=='FILES_VERIFIED'||!h.edgeId||h.sourceEventId!==e.sourceEventId||!tasks.has(h.fromTaskId)||!tasks.has(h.toTaskId)||h.fromTaskId===h.toTaskId||h.toTaskId!==e.taskId||!e.artifactIds.length)throw Error('PRESENTATION_HANDOFF_INVALID');
  }else if(e.handoff!==null||e.scope==='HANDOFF')throw Error('PRESENTATION_UNVERIFIED_HANDOFF');
  ids.add(e.eventId);sequence=e.sequence;
 }
}

/** One display session. Keep this instance across purpose, skin and locale changes. */
export class PresentationSession {
 private saved:PresentationSnapshot|null=null;
 private readonly seen=new Set<string>();
 private fault:string|null=null;
 get snapshot():PresentationSnapshot|null{return this.saved&&this.fault?{...this.saved,liveEvidence:'RESYNC_REQUIRED',liveReasonCode:this.fault}:this.saved;}
 get needsResync(){return this.fault!==null;}
 invalidate(reason='NOTIFICATION_DISCONNECTED'){this.fault=reason;}
 hydrate(s:PresentationSnapshot){
  try{validatePresentation(s);}catch{this.invalidate('INVALID_SNAPSHOT');return;}
  if(this.saved?.jobId===s.jobId&&(s.revision<this.saved.revision||s.eventCursor<this.saved.eventCursor)){this.invalidate('STALE_RESYNC');return;}
  this.saved=s;this.fault=null;s.events.forEach(e=>this.seen.add(e.sourceEventId));
 }
 apply(n:PresentationNotice,animate=true):readonly PresentationEvent[]{
  if(!this.saved)return [];
  if(n.contractVersion!==1||![n.revision,n.previousRevision,n.eventCursor,n.previousCursor].every(integer)||n.previousRevision>=n.revision||n.previousCursor>n.eventCursor){this.invalidate('INVALID_NOTIFICATION');return [];}
  if(n.revision<=this.saved.revision)return []; // stale or duplicate cannot replace saved state
  const s=n.snapshots.find(s=>s.jobId===this.saved?.jobId);
  if(!s||s.revision!==n.revision||s.eventCursor!==n.eventCursor){this.invalidate('NOTIFICATION_SNAPSHOT_MISMATCH');return [];}
  try{validatePresentation(s);}catch{this.invalidate('INVALID_NOTIFICATION');return [];}
  if(this.fault)return [];
  if(n.previousCursor!==this.saved.eventCursor||n.previousRevision!==this.saved.revision||n.eventCursor<this.saved.eventCursor){this.invalidate('CURSOR_OR_REVISION_GAP');return [];}
  const old=new Map(this.saved.events.map(e=>[e.eventId,e]));
  if(this.saved.events.some(e=>!s.events.some(v=>v.eventId===e.eventId&&v.sequence===e.sequence))||s.events.some(e=>old.has(e.eventId)&&JSON.stringify(old.get(e.eventId))!==JSON.stringify(e))){
   // File revalidation can downgrade an old receipt/success; fetch an authoritative
   // full frame and suppress all animation while reconciling that change.
   this.invalidate('SAVED_EVIDENCE_CHANGED');return [];
  }
  const fresh=animate&&this.saved.liveEvidence==='CURRENT'&&s.liveEvidence==='CURRENT'?s.events.filter(e=>e.sequence>this.saved!.eventCursor&&!this.seen.has(e.sourceEventId)&&e.status==='HANDOFF'):[];
  s.events.forEach(e=>this.seen.add(e.sourceEventId));this.saved=s;return fresh;
 }
}

/** Uses the existing CoreClient; this interface has no mutation/approval action. */
export async function openPresentation(client:CoreClient,jobId:string,onUpdate:(snapshot:PresentationSnapshot|null,handoffs:readonly PresentationEvent[])=>void){
 const session=new PresentationSession();let closed=false,syncing=false,subscribed=false;let queue:PresentationNotice[]=[];let off:(()=>void)|undefined;
 const publish=(handoffs:readonly PresentationEvent[]=[])=>{if(!closed)onUpdate(session.snapshot,handoffs);};
 async function resync(){
  if(closed||syncing)return;syncing=true;queue=[];
  try{
   if(!client.getPresentation)throw Error('PRESENTATION_UNAVAILABLE');
   const frame=await client.getPresentation(jobId);if(closed)return;session.hydrate(frame);
   // Anything observed during hydration is historical for animation purposes.
   for(const notice of queue)session.apply(notice,false);
   if(!subscribed)session.invalidate('SUBSCRIBE_FAILED');
  }catch{session.invalidate('SNAPSHOT_RECONCILIATION_FAILED');}finally{syncing=false;publish();}
 }
 const receive=(n:PresentationNotice)=>{if(closed)return;if(syncing){queue.push(n);return;}const handoffs=session.apply(n);publish(handoffs);if(session.needsResync)void resync();};
 async function subscribe(){
  subscribed=false;let failed=false;
  try{if(!client.subscribePresentation)throw Error('PRESENTATION_UNAVAILABLE');off=await client.subscribePresentation(receive,()=>{failed=true;subscribed=false;session.invalidate('NOTIFICATION_DISCONNECTED');publish();});subscribed=!failed;}catch{session.invalidate('SUBSCRIBE_FAILED');}
 }
 await subscribe();
 // Subscribe first. Buffer callbacks until the initial authoritative fetch.
 await resync();
 return {session,resync,reconnect:async()=>{if(closed)return;off?.();await subscribe();await resync();},disconnect:()=>{subscribed=false;session.invalidate('NOTIFICATION_DISCONNECTED');publish();},close:()=>{closed=true;off?.();}};
}
