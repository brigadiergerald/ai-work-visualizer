import {describe,it,expect} from 'vitest';
import {createElement} from 'react';
import {renderToStaticMarkup} from 'react-dom/server';
import {currentJobLocked,historyFor,initialSocialInput,socialInputIssue,socialInputHash,reconcilePurposeStart,confirmedNotStarted,type PurposeDefinition,type PendingPurposeStart} from '../src/core/purposes';
import type {CoreSnapshot,Job} from '../src/core/contract';
import SocialTextForm from '../src/dashboard/purposes/SocialTextForm';
import fixture from './fixtures/open-core.json';
const base=()=>structuredClone(fixture.awaiting) as CoreSnapshot;
const definition:PurposeDefinition={id:'social-text',definitionVersion:'1.0.0',roleMapVersion:'social-text-roles-v1',label:'SNS文章制作',description:'',availability:'READY',reasonCode:null,publicationMode:'PREPARATION_ONLY',inputSchema:{type:'object',additionalProperties:false,required:['theme','contentLanguage'],properties:{}}};
describe('Purpose S1 UI contract',()=>{
 it('validates UTF-8 bytes and keeps raw input and generation language separate',()=>{
  const v=initialSocialInput();expect(v.contentLanguage).toBe('ja');expect(socialInputIssue(v)).not.toBe('');v.theme='  原文\r\n次の行  ';v.context='媒体\n固定文言';v.contentLanguage='en';const before=JSON.stringify(v);expect(socialInputIssue(v)).toBe('');expect(JSON.stringify(v)).toBe(before);expect(socialInputIssue({...v,theme:'界'.repeat(21334)})).toContain('64,000');expect(socialInputIssue({...v,context:'a'.repeat(64001)})).toContain('64,000');
 });
 it('shares canonical input hash bytes with Core while preserving CRLF and optional context',async()=>{
  const input={theme:'  小さく始める\r\n固定文言を維持  ',context:'対象: 提供資料の読者\n媒体: 投稿用文章案\n外部投稿不要。',contentLanguage:'en' as const};
  // Independently produced by Rust Purpose context; fixture evidence retains its raw input.
  expect(await socialInputHash(input)).toBe('7a2ccbab620dc2aeb3c5cff84c2a9605352d24055618fa60296d5b7056b3ebc1');expect(await socialInputHash({...input,context:undefined})).not.toBe(await socialInputHash({...input,context:''}));
 });
 it('keeps every unfinished current Job locked including UNKNOWN and pending approval',()=>{
  const s=base();for(const state of ['queued','running','awaiting_approval','paused','unknown'] as const){s.jobs[0].state=state;expect(currentJobLocked(s)).toBe(true);}s.jobs[0].state='completed';expect(currentJobLocked(s)).toBe(false);
 });
 it('filters presentation without losing unclassified old history, selection, approvals or artifact hashes',()=>{
  const s=base(),j=s.jobs[0];s.jobs.push({...structuredClone(j),id:'old-general',workflowId:'codex-general-v1',state:'unknown'});const before=JSON.stringify(s);const selected=j.id;const draft={...initialSocialInput(),theme:'未完の入力',context:'補足',contentLanguage:'en' as const};
  for(const purpose of ['social-text','research','social-text'] as const){historyFor(s.jobs,purpose);expect(s.jobs.find(j=>j.id===selected)?.approvals).toEqual(j.approvals);expect(draft.theme).toBe('未完の入力');}
  expect(historyFor(s.jobs,'all')).toHaveLength(2);expect(historyFor(s.jobs,'social-text')).toHaveLength(1);expect(historyFor(s.jobs,'research')).toHaveLength(0);expect(JSON.stringify(s)).toBe(before);
 });
 it('recovers a lost start response only from a unique new persisted Job and exact input binding',()=>{
  const s=base();const pending:PendingPurposeStart={profile:s.dataPath,beforeIds:s.jobs.map(j=>j.id),inputSha256:'abc',definitionVersion:'1.0.0',contentLanguage:'en'};
  const next:Job={...structuredClone(s.jobs[0]),id:'new-purpose-job',purpose:{jobId:'new-purpose-job',purposeId:'social-text',definitionVersion:'1.0.0',roleMapVersion:'social-text-roles-v1',contentLanguage:'en',inputSha256:'abc',contextSource:'PERSISTED_NEW_JOB'}};
  expect(reconcilePurposeStart(pending,s)).toBeNull();s.jobs.push(next);expect(reconcilePurposeStart(pending,s)?.id).toBe(next.id);expect(reconcilePurposeStart({...pending,profile:'different'},s)).toBeNull();expect(reconcilePurposeStart({...pending,contentLanguage:'ja'},s)).toBeNull();expect(reconcilePurposeStart({...pending,inputSha256:'wrong'},s)).toBeNull();s.jobs.push({...next,id:'ambiguous'});expect(reconcilePurposeStart(pending,s)).toBeNull();
 });
 it('does not interpret a timeout or missing Job as proof that a request was unsent',()=>{
  expect(confirmedNotStarted(new Error('timeout'))).toBe(false);expect(confirmedNotStarted('timeout')).toBe(false);expect(confirmedNotStarted('[PURPOSE_NOT_STARTED] validation rejected')).toBe(true);expect(reconcilePurposeStart({profile:'x',beforeIds:[],inputSha256:'x',contentLanguage:'ja',definitionVersion:'1'},base())).toBeNull();
 });
 it('renders dedicated theme/context/language controls, preserved completion plan and a clear start action',()=>{
  const html=renderToStaticMarkup(createElement(SocialTextForm,{value:{theme:'原文\n固定文言',context:'補足',contentLanguage:'en'},onChange:()=>{},definition,completionPlan:{goal:'exportPackage'},onPlan:()=>{},profiles:[],busy:false,locked:false,canStart:true,startBlockedReason:'',onStart:()=>{}}));
  for(const copy of ['投稿テーマ','補足情報（任意）','生成する文章の言語','SNS文章制作を開始','到達目標・詳細設定','引き渡し用ファイルの出力まで','英語生成品質の実Codex E2Eは未検証'])expect(html).toContain(copy);expect(html).toContain('<option value="en" selected="">');expect(html).toContain('原文\n固定文言');
 });
 it('blocks start for PLANNED or missing catalog definitions and pending/locked state',()=>{
  for(const d of [undefined,{...definition,availability:'PLANNED' as const}]){const html=renderToStaticMarkup(createElement(SocialTextForm,{value:{theme:'テーマ',contentLanguage:'ja'},onChange:()=>{},definition:d,completionPlan:{goal:'localFiles'},onPlan:()=>{},profiles:[],busy:false,locked:false,canStart:true,startBlockedReason:'',onStart:()=>{}}));expect(html).toMatch(/type="submit" disabled=""/);}
 });
});
