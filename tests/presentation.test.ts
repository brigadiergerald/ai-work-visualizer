import {it,expect} from 'vitest';
import {PresentationSession,openPresentation,validatePresentation,type PresentationEvent,type PresentationSnapshot,type PresentationNotice} from '../src/core/presentation';
import type {CoreClient} from '../src/core/contract';
import rustAwaiting from '../tests/fixtures/presentation-awaiting.json';
import rustCompleted from '../tests/fixtures/presentation-completed.json';
const ev=(id:string,sequence:number,jobId='job'):PresentationEvent=>({contractVersion:1,eventId:id,sourceEventId:id,jobId,taskId:'to',purposeId:'social-text',phase:'writing',status:'HANDOFF',roleId:'writer',timestamp:'2026-10-29T00:00:00Z',artifactIds:['artifact:0'],approvalId:null,sequence,scope:'HANDOFF',mode:'real',legacyStatus:'queued',reasonCode:null,handoff:{edgeId:'plan-writing',fromTaskId:'from',toTaskId:'to',sourceEventId:id,verification:'FILES_VERIFIED'}});
const snap=(revision:number,eventCursor:number,events:PresentationEvent[]=[],jobId='job'):PresentationSnapshot=>({contractVersion:1,jobId,revision,eventCursor,events,liveEvidence:'CURRENT',liveReasonCode:null,purpose:{jobId,purposeId:'social-text',definitionVersion:'1.0.0',roleMapVersion:'social-text-roles-v1',contentLanguage:'ja',inputSha256:'fixture',contextSource:'PERSISTED_NEW_JOB'},roleMapVersion:'social-text-roles-v1',roles:[{id:'planner',purposeId:'social-text',labelKey:'roles.social-text.planner',actor:'codex'},{id:'writer',purposeId:'social-text',labelKey:'roles.social-text.writer',actor:'codex'}],tasks:[{taskId:'from',nodeId:'plan',attempt:1,roleId:'planner',status:'SUCCEEDED',legacyStatus:'completed',reasonCode:null},{taskId:'to',nodeId:'writing',attempt:1,roleId:'writer',status:'RUNNING',legacyStatus:'running',reasonCode:null}],jobStatus:'RUNNING',legacyJobStatus:'running',artifacts:[{artifactId:'artifact:0',name:'plan.json',sha256:'fixture',bytes:1,integrity:'verified'}]});
const notice=(previous:PresentationSnapshot,next:PresentationSnapshot):PresentationNotice=>({contractVersion:1,previousRevision:previous.revision,previousCursor:previous.eventCursor,revision:next.revision,eventCursor:next.eventCursor,snapshots:[next]});
function setup(){const session=new PresentationSession(),first=snap(1,1,[ev('old',1)]);session.hydrate(first);return {session,first};}
it('hydrates, restarts and changes selected history without replaying past handoffs',()=>{
 const {session,first}=setup();expect(session.snapshot?.events).toHaveLength(1);expect(session.apply(notice(first,first))).toEqual([]);
 const other=snap(2,3,[ev('other',3,'other')],'other');session.hydrate(other);session.hydrate(snap(3,3,first.events.slice()));expect(session.apply(notice(snap(3,3,first.events.slice()),snap(4,4,[...first.events,ev('fresh',4)]))).map(e=>e.eventId)).toEqual(['fresh']);
 const reboot=new PresentationSession();reboot.hydrate(session.snapshot!);expect(reboot.apply(notice(first,session.snapshot!))).toEqual([]);
});
it('plays a new verified source event exactly once per session and ignores duplicate/out-of-order frames',()=>{
 const {session,first}=setup(),second=snap(2,2,[...first.events,ev('new',2)]),n=notice(first,second);expect(session.apply(n).map(e=>e.eventId)).toEqual(['new']);expect(session.apply(n)).toEqual([]);expect(session.apply(notice(snap(0,0),first))).toEqual([]);expect(session.snapshot?.revision).toBe(2);
});
it('keeps source deduplication across skin, purpose and locale rehydration',()=>{
 const {session,first}=setup(),second=snap(2,2,[...first.events,ev('new',2)]);session.apply(notice(first,second));session.hydrate(second);expect(session.apply(notice(second,snap(3,3,[...second.events])))).toEqual([]);
});
it('detects a cursor gap, latches RESYNC_REQUIRED and suppresses future animation until full reconciliation',()=>{
 const {session,first}=setup(),second=snap(3,3,[...first.events,ev('lost',2),ev('after-gap',3)]);expect(session.apply({...notice(first,second),previousCursor:2,previousRevision:2})).toEqual([]);expect(session.snapshot?.liveEvidence).toBe('RESYNC_REQUIRED');expect(session.snapshot?.revision).toBe(1);expect(session.apply(notice(first,second))).toEqual([]);session.hydrate(second);expect(session.needsResync).toBe(false);expect(session.apply(notice(second,snap(4,4,[...second.events,ev('next',4)]))).map(e=>e.eventId)).toEqual(['next']);
});
it('detects a revision gap even when no event was written and never interprets silence as success/failure',()=>{
 const {session,first}=setup();session.apply({...notice(first,snap(3,1,first.events.slice())),previousRevision:2});expect(session.snapshot?.liveEvidence).toBe('RESYNC_REQUIRED');expect(first.jobStatus).toBe('RUNNING');
});
it('marks disconnect and failed reconciliation without mutating the saved Job state',()=>{
 const {session}=setup();session.invalidate();expect(session.snapshot?.liveEvidence).toBe('RESYNC_REQUIRED');expect(session.snapshot?.jobStatus).toBe('RUNNING');session.hydrate(snap(0,0));expect(session.needsResync).toBe(true);
});
it('rejects wrong-job, duplicate IDs, backwards sequences and unsupported versions',()=>{
 for(const s of [snap(1,1,[ev('wrong',1,'other')]),snap(2,2,[ev('same',1),ev('same',2)]),snap(2,2,[ev('later',2),ev('earlier',1)]),{...snap(1,1),contractVersion:2}])expect(()=>validatePresentation(s as PresentationSnapshot)).toThrow();
});
it('does not accept started-only, missing edge/artifact/task or forged handoff scope as receipt',()=>{
 const base=ev('h',1);for(const e of [{...base,handoff:null},{...base,artifactIds:[]},{...base,handoff:{...base.handoff!,edgeId:''}},{...base,handoff:{...base.handoff!,fromTaskId:'missing'}},{...base,status:'RUNNING',handoff:null}])expect(()=>validatePresentation(snap(1,1,[e as PresentationEvent]))).toThrow();
});
it('suppresses live handoffs when formal evidence is UNKNOWN and accepts paused/cancelled IDLE',()=>{
 const {session,first}=setup(),second={...snap(2,2,[...first.events,ev('new',2)]),liveEvidence:'UNKNOWN' as const,liveReasonCode:'FORMAL_RECONCILIATION_REQUIRED'};expect(session.apply(notice(first,second))).toEqual([]);expect(session.snapshot?.liveEvidence).toBe('UNKNOWN');for(const status of ['paused','cancelled']){const e={...ev(status,3),status:'IDLE' as const,handoff:null,scope:'TASK' as const,legacyStatus:status,reasonCode:status.toUpperCase()};expect(()=>validatePresentation(snap(3,3,[e]))).not.toThrow();}
});
it('allows per-job sequence gaps caused by other jobs when the global predecessor cursor matches',()=>{
 const {session,first}=setup(),second=snap(2,12,[...first.events,ev('next-for-this-job',12)]);expect(session.apply(notice(first,second))).toHaveLength(1);expect(session.needsResync).toBe(false);
});
it('resyncs when revalidation downgrades saved evidence and never replays the old source',()=>{
 const {session,first}=setup();const downgraded={...first.events[0],status:'UNKNOWN' as const,handoff:null,scope:'TASK' as const,reasonCode:'HANDOFF_EVIDENCE_MISSING_OR_INVALID'};const next=snap(2,1,[downgraded]);expect(session.apply(notice(first,next))).toEqual([]);expect(session.needsResync).toBe(true);session.hydrate(next);expect(session.snapshot?.events[0].status).toBe('UNKNOWN');
});
function client(get:()=>Promise<PresentationSnapshot>,subscribe:(fn:(n:PresentationNotice)=>void,onError?:(e:unknown)=>void)=>Promise<()=>void>):CoreClient{return {environment:'offline-fixture',getSnapshot:async()=>{throw Error('v1 must remain independent')},subscribe:async()=>()=>{},execute:async()=>{throw Error('no mutation')},getPresentation:get,subscribePresentation:subscribe};}
it('subscribes before snapshot and buffers hydration races without any historical animation',async()=>{
 const first=snap(1,1,[ev('old',1)]),next=snap(2,2,[...first.events,ev('raced',2)]);let fn:(n:PresentationNotice)=>void=()=>{};const order:string[]=[],animated:string[]=[];
 const c=client(async()=>{order.push('snapshot');fn(notice(first,next));return first;},async cb=>{order.push('subscribe');fn=cb;return()=>{order.push('close')}});
 const controller=await openPresentation(c,'job',(_s,h)=>animated.push(...h.map(e=>e.eventId)));expect(order.slice(0,2)).toEqual(['subscribe','snapshot']);expect(controller.session.snapshot?.revision).toBe(2);expect(animated).toEqual([]);fn(notice(next,snap(3,3,[...next.events,ev('live',3)])));expect(animated).toEqual(['live']);controller.close();expect(order.at(-1)).toBe('close');
});
it('keeps a saved snapshot visible after subscribe failure with live RESYNC_REQUIRED',async()=>{
 const c=client(async()=>snap(1,1,[ev('old',1)]),async()=>{throw Error('injected subscribe failure')});const view=await openPresentation(c,'job',()=>{});expect(view.session.snapshot?.events).toHaveLength(1);expect(view.session.snapshot?.liveEvidence).toBe('RESYNC_REQUIRED');view.close();
});
it('marks snapshot failure and a later transport disconnection safely',async()=>{
 let fail=true,transportError:(e:unknown)=>void=()=>{};const c=client(async()=>{if(fail)throw Error('injected read failure');return snap(1,1,[ev('old',1)]);},async(_fn,onError)=>{transportError=onError!;return()=>{}});const view=await openPresentation(c,'job',()=>{});expect(view.session.needsResync).toBe(true);fail=false;await view.resync();expect(view.session.snapshot?.liveEvidence).toBe('CURRENT');transportError(Error('disconnected'));expect(view.session.snapshot?.liveEvidence).toBe('RESYNC_REQUIRED');view.close();
});
it('requires a new successful subscription after disconnect and rehydrates without replay',async()=>{
 let transportError:(e:unknown)=>void=()=>{};let subscriptions=0;const c=client(async()=>snap(1,1,[ev('old',1)]),async(_fn,onError)=>{subscriptions++;transportError=onError!;return()=>{}});const animations:string[]=[];const view=await openPresentation(c,'job',(_s,h)=>animations.push(...h.map(e=>e.eventId)));transportError(Error('lost'));await view.resync();expect(view.session.snapshot?.liveEvidence).toBe('RESYNC_REQUIRED');await view.reconnect();expect(subscriptions).toBe(2);expect(view.session.snapshot?.liveEvidence).toBe('CURRENT');expect(animations).toEqual([]);view.close();
});
it('consumes actual Rust Store snapshots with the same TS contract and no protocol IDs',()=>{
 const awaiting=rustAwaiting as unknown as PresentationSnapshot,completed=rustCompleted as unknown as PresentationSnapshot;
 expect(()=>validatePresentation(awaiting)).not.toThrow();expect(()=>validatePresentation(completed)).not.toThrow();expect(awaiting.jobStatus).toBe('AWAITING_APPROVAL');expect(completed.jobStatus).toBe('SUCCEEDED');expect(completed.events.filter(e=>e.status==='HANDOFF')).toHaveLength(4);expect(completed.tasks.find(t=>t.nodeId==='sns_review')?.status).toBe('SUCCEEDED');expect(completed.roles.find(r=>r.id==='verifier')?.actor).toBe('host');
 const session=new PresentationSession();session.hydrate(completed);expect(session.apply({...notice(awaiting,completed)})).toEqual([]);for(const forbidden of ['threadId','turnId','requestId','workInstruction'])expect(JSON.stringify(completed)).not.toContain(forbidden);
});
