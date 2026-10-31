import {afterEach,expect,it} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import DecisionRecords from '../src/dashboard/DecisionRecords';
import {setUiLocale} from '../src/i18n';
import type {Job} from '../src/core/contract';
import type {DecisionRecord} from '../src/core/decisions';
afterEach(()=>setUiLocale('ja'));
const record=(kind:DecisionRecord['kind']='theme'):DecisionRecord=>({kind,jobId:'job-a',nodeId:'plan',taskId:'task-plan',attempt:1,roleId:'director',stageState:'completed',status:'RECORDED',recordedAt:'2026-10-30T10:00:00Z',source:{name:'plan.json',path:'C:/output/plan.json',sha256:'a'.repeat(64),bytes:200,integrity:'verified'},note:{summary:'選択したテーマの原文',reason:'同じ企画応答で記録した理由',details:['外部調査なし']},inspection:null});
const job=(records:DecisionRecord[]=[]):Job=>({id:'job-a',state:'completed',stages:[],decisions:{version:1,jobId:'job-a',records,humanContentApproval:'NOT_PERFORMED',publication:'NOT_PERFORMED'}} as unknown as Job);
const render=(j:Job|null)=>renderToStaticMarkup(<DecisionRecords job={j} busy={false} onOpen={()=>{throw Error('Renderer may not execute actions');}}/>);
it('renders literal saved choices and evidence in Japanese and English without translating contentLanguage',()=>{
 for(const locale of ['ja','en'] as const){setUiLocale(locale);const html=render(job([record()]));expect(html).toContain('選択したテーマの原文');expect(html).toContain('同じ企画応答で記録した理由');expect(html).toContain('plan.json');expect(html).toContain('task-plan');expect(html).toContain(locale==='ja'?'判断の記録':'Decision records');expect(html).not.toContain('UNKNOWN (director)');}
});
it('old Jobs have no inferred reason, and another Job cannot supply a record',()=>{
 for(const j of [job(),job([{...record(),jobId:'another-job'}]),{...job([record()]),decisions:{...job().decisions!,jobId:'another-job'}}]){const html=render(j);expect(html).toContain('判断理由の記録なし');expect(html).not.toContain('選択したテーマの原文');}
});
it('running, failed, UNKNOWN, and corrupt sources cannot be shown as completed decisions',()=>{
 for(const state of ['running','failed','disconnected']){const html=render(job([{...record(),stageState:state,status:'NO_RECORD',note:null}]));expect(html).toContain(state==='disconnected'?'UNKNOWN':state==='failed'?'失敗':'実行中');expect(html).not.toContain('選択したテーマの原文');}
 const html=render(job([{...record(),status:'SOURCE_FAILED',source:{...record().source!,integrity:'failed'}}]));expect(html).not.toContain('同じ企画応答で記録した理由');expect(html).toContain('証拠ファイルの照合失敗');
});
it('separates measured checks, unverified semantics, pending human approval and publication',()=>{
 const r:DecisionRecord={...record('inspection'),nodeId:'qa',roleId:'inspector',note:null,inspection:{version:1,status:'MACHINE_CHECKS_PASSED',basis:'HOST_VERIFY_PRODUCT',checks:[{criterion:'CHARACTER_COUNT',result:'MEASURED_ONLY',observed:124}],unverified:['FACTS_EXPERIENCES_NUMBERS'],nextStageBasis:'VERIFIED_LOCAL_FILES_ONLY',humanContentApproval:'NOT_PERFORMED',publication:'NOT_PERFORMED'}};
 const j=job([r]);j.state='awaiting_approval';j.decisions!.humanContentApproval='PENDING';const html=render(j);expect(html).toContain('人間の内容承認待ち');expect(html).toContain('外部公開：未実施');expect(html).toContain('計測のみ・条件適合は未検証');expect(html).toContain('124');expect(html).toContain('事実・体験・数値・引用の外部照合');expect(html).toContain('disabled');
});
it('empty state is explicit and summaries escape saved text',()=>{expect(render(null)).toContain('Jobを開始すると');const r=record();r.note!.summary='<script>untrusted()</script>';expect(render(job([r]))).not.toContain('<script>');});
it('uses only formally recorded valid timestamps in compact rows and keeps exact reasons in details',()=>{
 const r=record(),html=render(job([r]));
 expect(html).toContain('dateTime="2026-10-30T10:00:00Z"');
 const summary=html.match(/class="decision-entry"><summary>(.*?)<\/summary>/)?.[1]??'';
 expect(summary).toContain('テーマ選定');expect(summary).not.toContain(r.note!.summary);expect(summary).not.toContain(r.note!.reason);
 expect(html).toContain(r.note!.summary);expect(html).toContain(r.note!.reason);
 for(const [value,status] of [[null,'時刻なし'],['not-a-time','時刻不正'],['2026-02-30T10:00:00Z','時刻不正'],['1','時刻不正']] as const){const result=render(job([{...r,recordedAt:value}]));expect(result).toContain(status);expect(result).not.toContain('<time');}
 const invalid=render(job([{...r,status:'SOURCE_FAILED'}]));expect(invalid).toContain('未記録');expect(invalid).not.toContain('<time');
});
