import {it,expect} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import StageScene from '../src/dashboard/StageScene';
import StageActor from '../src/dashboard/StageActor';
import StageActivityLog from '../src/dashboard/StageActivityLog';
import {stationsFor,connectionsFor,sceneLayout,spriteRow,stationStatus} from '../src/dashboard/stage-visuals';
import {spriteSpec,builtInPeople} from '../src/dashboard/stage-assets';
import type {PresentationSnapshot,PresentationStatus} from '../src/core/presentation';
import awaiting from '../tests/fixtures/presentation-awaiting.json';
const snapshot=()=>structuredClone(awaiting) as PresentationSnapshot;
it('shows four people and follows inspection/save through completion',()=>{
 const base=snapshot();
 const task=(nodeId:string,roleId:string,status:PresentationStatus,attempt=1)=>({taskId:`${nodeId}-${attempt}`,nodeId,roleId,status,attempt,legacyStatus:status==='RUNNING'?'running':'completed',reasonCode:null});
 const current:PresentationSnapshot={...base,roleMapVersion:'social-text-roles-v2',liveEvidence:'CURRENT',jobStatus:'RUNNING',roles:['researcher','director','creator','inspector'].map(id=>({id,purposeId:'social-text',labelKey:`roles.social-text.${id}`,actor:id==='inspector'?'host':'codex'})),tasks:[task('research','researcher','SUCCEEDED'),task('plan','director','SUCCEEDED'),task('writing','creator','SUCCEEDED'),task('qa','inspector','RUNNING')]};
 const stations=stationsFor(current);expect(stations).toHaveLength(4);expect(stations.filter(s=>s.moving).map(s=>s.role.id)).toEqual(['inspector']);expect(stations[3].task?.nodeId).toBe('qa');
 const html=renderToStaticMarkup(<StageScene snapshot={current} handoffs={[]} visible hasJob/>);expect((html.match(/class="stage-station /g)??[])).toHaveLength(4);expect(html).toContain('投稿文の品質検査');
 const saving={...current,tasks:[...current.tasks.map(t=>({...t,status:'SUCCEEDED' as const})),task('delivery','inspector','RUNNING')]};expect(stationsFor(saving)[3].task?.nodeId).toBe('delivery');expect(stationsFor(saving)[3].moving).toBe(true);
 const done={...saving,jobStatus:'SUCCEEDED' as const,tasks:saving.tasks.map(t=>({...t,status:'SUCCEEDED' as const}))};expect(stationsFor(done).every(s=>!s.moving)).toBe(true);expect(stationsFor({...saving,liveEvidence:'RESYNC_REQUIRED'}).every(s=>!s.moving)).toBe(true);
 expect(builtInPeople.researcher.sprite).toBe(builtInPeople.planner.sprite);expect(builtInPeople.director.sprite).toBe(builtInPeople.reviewer.sprite);expect(builtInPeople.creator.sprite).toBe(builtInPeople.writer.sprite);
 const retry={...saving,tasks:[...saving.tasks,task('writing','creator','RUNNING',2)]};expect(stationsFor(retry)[2].task?.nodeId).toBe('writing');expect(stationsFor(retry)[2].task?.attempt).toBe(2);
});
it('ties the whole-person motion and static emphasis to the existing CURRENT Role projection only',()=>{
 const source=snapshot(),before=JSON.stringify(source);
 const run={...source,liveEvidence:'CURRENT' as const,jobStatus:'RUNNING' as const,tasks:source.tasks.map(t=>({...t,status:'RUNNING' as const}))};
 for(const liveEvidence of ['CURRENT','UNKNOWN','RESYNC_REQUIRED'] as const)
 for(const jobStatus of ['IDLE','RUNNING','AWAITING_APPROVAL','SUCCEEDED','FAILED','UNKNOWN'] as const)
 for(const status of ['IDLE','RUNNING','AWAITING_APPROVAL','SUCCEEDED','FAILED','UNKNOWN'] as const)
 for(const visible of [true,false]){
  const stations=stationsFor({...run,liveEvidence,jobStatus,tasks:run.tasks.map(t=>({...t,status}))},visible);
  for(const station of stations){
   const html=renderToStaticMarkup(<StageActor station={station}/>);
   const active=visible&&liveEvidence==='CURRENT'&&jobStatus==='RUNNING'&&station.status==='RUNNING'&&station.role.actor!=='human';
   expect(html.includes('is-working')).toBe(active);
   // The wrapper owns translation; the original PNG and background frame remain inside.
   expect(html).toContain('class="actor-motion"><div class="workshop-sprite"');
   expect(html).toContain(station.sprite!);
   expect(html).toContain('class="station-state"');
  }
 }
 expect(JSON.stringify(source)).toBe(before);
});
it('renders every normalized state with text, task scope and immutable role/actor identity',()=>{
 for(const status of ['IDLE','RUNNING','AWAITING_APPROVAL','SUCCEEDED','FAILED','UNKNOWN'] as const){
  const s=snapshot();const next={...s,jobStatus:status,tasks:s.tasks.map(t=>({...t,status}))};
  const html=renderToStaticMarkup(<StageScene snapshot={next} visible handoffs={[]} hasJob/>);
  expect(html).not.toContain('小さな作業工房');expect(html).toContain('Codexの工程');expect(html).toContain('人間が確認・承認');expect(html).toContain('工程の状態');
  expect(html).not.toContain('handoff-spark');expect(stationsFor(next).every(v=>v.moving===(status==='RUNNING'&&v.status==='RUNNING'&&v.role.actor!=='human'))).toBe(true);
 }
});
it('freezes running sprites and handoffs on UNKNOWN, RESYNC_REQUIRED, hidden view and approval',()=>{
 const s=snapshot(),run={...s,jobStatus:'RUNNING' as const,tasks:s.tasks.map(t=>({...t,status:'RUNNING' as const}))};
 for(const liveEvidence of ['UNKNOWN','RESYNC_REQUIRED'] as const){expect(stationsFor({...run,liveEvidence}).every(v=>!v.moving)).toBe(true);}
 expect(stationsFor(run,false).every(v=>!v.moving)).toBe(true);
 expect(stationsFor({...run,jobStatus:'AWAITING_APPROVAL'}).every(v=>!v.moving)).toBe(true);
});
it('maps only the six actual sheet rows; no generated frames, assets or role count dependency',()=>{
 expect(['IDLE','RUNNING','HANDOFF','SUCCEEDED','FAILED','UNKNOWN','AWAITING_APPROVAL'].map(v=>spriteRow(v as PresentationStatus))).toEqual([0,1,0,3,4,5,2]);
 const s=snapshot();const custom={id:'custom',purposeId:'custom',labelKey:'custom',actor:'host' as const};
 expect(stationsFor({...s,roles:[custom],tasks:[]})).toMatchObject([{label:'custom',sprite:null,status:'IDLE'}]);
 expect(stationsFor({...s,roles:[],tasks:[]})).toEqual([]);
});
it('selects highest real attempt, preserves paused/cancelled and does not promote task success to Job success',()=>{
 const s=snapshot(),t=s.tasks[0];const stations=stationsFor({...s,jobStatus:'RUNNING',tasks:[{...t,attempt:2,status:'IDLE',legacyStatus:'paused'},t]});
 expect(stationStatus(stations[0])).toBe('一時停止');expect(stationStatus({...stations[0],task:{...t,status:'IDLE',legacyStatus:'cancelled'}})).toBe('停止済み');
 const html=renderToStaticMarkup(<StageScene snapshot={s} visible handoffs={[]} hasJob/>);expect(html).toContain('人間の承認待ち');expect(html).not.toContain('保存されたJob：<strong>完了');
});
it('draws only Core verified edge/task relationships; hydration creates no light',()=>{
 const s=snapshot(),stations=stationsFor(s),edges=connectionsFor(s,stations);
 expect(edges.length).toBeGreaterThan(0);expect(connectionsFor({...s,events:[]},stations)).toEqual([]);
 const html=renderToStaticMarkup(<StageScene snapshot={s} visible handoffs={[]} hasJob/>);expect(html).toContain('確認済み受け渡しの接続線');expect(html).not.toContain('<animateMotion');
 const handoff=s.events.find(e=>e.status==='HANDOFF')!;
 expect(renderToStaticMarkup(<StageScene snapshot={{...s,liveEvidence:'UNKNOWN'}} visible handoffs={[handoff]} hasJob/>)).not.toContain('<animateMotion');
});
it('lays out arbitrary role counts within width without reducing sprite scale',()=>{
 for(const width of [200,320,600,830,1200])for(const count of [1,2,5,8,13]){
  const l=sceneLayout(width,count);expect(l.positions).toHaveLength(count);expect(l.positions.every(p=>p.x>=0&&p.x<=width&&p.y<=l.height)).toBe(true);
 }
});
it('keeps the native 48x72 cells at only integer 1x or 2x, with receipt rails below text',()=>{
 expect(spriteSpec).toEqual({cellWidth:48,cellHeight:72,columns:4,rows:6,scale:2});
 for(const width of [200,384,599,600,830,1200]){
  const l=sceneLayout(width,5);expect(l.scale).toBe(width<800?1:2);
  expect(l.positions.every(p=>p.y%l.rowHeight===l.rowHeight-24)).toBe(true);
  expect(l.cellWidth).toBeGreaterThanOrEqual(width<120?width:120);
 }
});
it('uses the approval row while human remains stationary even with RUNNING input',()=>{
 const s=snapshot();const run={...s,jobStatus:'RUNNING' as const,tasks:s.tasks.map(t=>({...t,status:'RUNNING' as const}))};
 expect(stationsFor(run).find(v=>v.role.id==='reviewer')?.moving).toBe(false);
 const html=renderToStaticMarkup(<StageScene snapshot={s} visible handoffs={[]} hasJob/>);
 expect(html).toContain('calc(var(--sprite-height) * -2)');
 expect(html).not.toContain('background-position-y:-192px');
});
it('fits the five standard people into one row in the S4.5 desktop and compact studios',()=>{
 for(const width of [626,806]){
  const layout=sceneLayout(width,5);
  expect(layout.columns).toBe(5);
  expect(new Set(layout.positions.map(p=>p.y)).size).toBe(1);
  expect(layout.cellWidth).toBeGreaterThanOrEqual(layout.scale===1?120:160);
 }
});
it('keeps Core log messages literal, escapes markup and labels empty state without fabricated events',()=>{
 const html=renderToStaticMarkup(<StageActivityLog logs={[{id:'1',at:'2026-10-29T00:00:00Z',state:'unknown',stageId:null,message:'元の文言 <script> & 不明',code:'SOURCE_CODE'}]}/>);
 expect(html).toContain('元の文言 &lt;script&gt; &amp; 不明');expect(html).toContain('実イベント');
 expect(renderToStaticMarkup(<StageActivityLog logs={[]}/>)).toContain('実イベントがここに表示されます');
});
