import {it,expect,vi} from 'vitest';
import {StagePresentationFeed,type StageFrame} from '../src/dashboard/stage-presentation';
import type {CoreClient} from '../src/core/contract';
import type {PresentationSnapshot,PresentationNotice} from '../src/core/presentation';
import awaiting from '../tests/fixtures/presentation-awaiting.json';
function setup(){
 const original=structuredClone(awaiting) as PresentationSnapshot;
 const receipt=original.events.find(e=>e.status==='HANDOFF')!;
 let snapshot={...original,eventCursor:receipt.sequence-1,revision:1,events:original.events.filter(e=>e.sequence<receipt.sequence)};
 let notice:(n:PresentationNotice)=>void=()=>{},fail:(e:unknown)=>void=()=>{};let closed=0,subscriptions=0;
 const c:CoreClient={environment:'offline-fixture',getSnapshot:async()=>{throw Error('unused')},subscribe:async()=>()=>{},execute:async()=>{throw Error('No mutation permitted')},getPresentation:async()=>snapshot,subscribePresentation:async(fn,onError)=>{notice=fn;fail=onError!;subscriptions++;return()=>{closed++;};}};
 let frame:StageFrame={snapshot:null,handoffs:[],visible:true};const feed=new StagePresentationFeed(v=>frame=v);
 return {feed,c,receipt,get frame(){return frame;},get closed(){return closed;},get subscriptions(){return subscriptions;},disconnect:()=>fail(Error('transport disconnected')),emit:(gap=false)=>{
  const before=snapshot;snapshot={...snapshot,revision:snapshot.revision+1,eventCursor:receipt.sequence,events:[...snapshot.events,receipt]};
  const n:PresentationNotice={contractVersion:1,previousRevision:gap?before.revision+5:before.revision,previousCursor:before.eventCursor,revision:snapshot.revision,eventCursor:snapshot.eventCursor,snapshots:[snapshot]};notice(n);return()=>notice(n);
 }};
}
it('connects via original openPresentation, displays hydration, plays one fresh receipt once and expires visually',async()=>{
 vi.useFakeTimers();const t=setup();await t.feed.select(t.c,'job');expect(t.frame.handoffs).toEqual([]);
 const duplicate=t.emit();expect(t.frame.handoffs).toHaveLength(1);duplicate();expect(t.frame.handoffs).toHaveLength(1);
 vi.advanceTimersByTime(1300);expect(t.frame.handoffs).toEqual([]);expect(t.frame.snapshot?.jobStatus).toBe('AWAITING_APPROVAL');t.feed.close();vi.useRealTimers();
});
it('does not replay on theme/purpose clearing, history selection, restart, resync or reconnect',async()=>{
 const t=setup();await t.feed.select(t.c,'job');t.emit();t.feed.clearHandoffs();expect(t.frame.handoffs).toEqual([]);
 await t.feed.reconnect();expect(t.frame.handoffs).toEqual([]);expect(t.subscriptions).toBe(2);
 await t.feed.select(t.c,'other-history');expect(t.frame.handoffs).toEqual([]);t.feed.close();await t.feed.select(t.c,'job');expect(t.frame.handoffs).toEqual([]);t.feed.close();
});
it('clears pulses and freezes on observed transport loss while retaining stored Job state',async()=>{
 const t=setup();await t.feed.select(t.c,'job');t.emit();t.disconnect();expect(t.frame.handoffs).toEqual([]);expect(t.frame.snapshot?.liveEvidence).toBe('RESYNC_REQUIRED');expect(t.frame.snapshot?.jobStatus).toBe('AWAITING_APPROVAL');await t.feed.reconnect();expect(t.frame.handoffs).toEqual([]);t.feed.close();
});
it('clears hidden/selected-away work, ignores callbacks from closed sessions and never replays when shown',async()=>{
 const t=setup();await t.feed.select(t.c,'job');t.feed.setVisible(false);const late=t.emit();expect(t.frame.handoffs).toEqual([]);t.feed.setVisible(true);await Promise.resolve();await Promise.resolve();expect(t.frame.handoffs).toEqual([]);
 await t.feed.select(t.c,null);late();expect(t.frame.snapshot).toBeNull();expect(t.frame.handoffs).toEqual([]);expect(t.closed).toBeGreaterThan(0);t.feed.close();
});
it('notification gap uses native session reconciliation and suppresses the missing receipt',async()=>{
 const t=setup();await t.feed.select(t.c,'job');t.emit(true);expect(t.frame.handoffs).toEqual([]);await Promise.resolve();await Promise.resolve();expect(t.frame.handoffs).toEqual([]);t.feed.close();
});
it('unavailable presentation never fabricates a running job or calls an action',async()=>{
 const t=setup();const c={...t.c,getPresentation:undefined,subscribePresentation:undefined};await t.feed.select(c,'job');expect(t.frame.snapshot).toBeNull();expect(t.frame.handoffs).toEqual([]);t.feed.close();
});
it('retains a transport loss observed while the initial authoritative read is still pending',async()=>{
 const t=setup();let release:(s:PresentationSnapshot)=>void=()=>{};
 const pending=new Promise<PresentationSnapshot>(resolve=>{release=resolve;});
 const c={...t.c,getPresentation:()=>pending};const selecting=t.feed.select(c,'job');
 await Promise.resolve();t.feed.disconnect();release(structuredClone(awaiting) as PresentationSnapshot);await selecting;
 expect(t.frame.snapshot?.liveEvidence).toBe('RESYNC_REQUIRED');expect(t.frame.handoffs).toEqual([]);t.feed.close();
});
