import {describe,it,expect} from 'vitest';
import {createElement} from 'react';
import {renderToStaticMarkup} from 'react-dom/server';
import {currentMock,planReady,type IntegrationProfile,type MockCheck} from '../src/core/integrations';
import {CompletionSelector,ConnectionSettings} from '../src/dashboard/IntegrationsPanel';
import type {CoreClient} from '../src/core/contract';
const p:IntegrationProfile={id:'f5b4f75f-9dbd-4f3f-8af4-82991d58121c',revision:2,label:'架空の接続',provider:'custom',transport:'webhook',endpoint:'https://example.invalid/hook',accountLabel:'架空アカウント',credentialRef:null,capabilities:['externalDraft']};
describe('optional integrations contract and controls',()=>{
 it('works without any profile and rejects stale or unsupported target plans',()=>{
  expect(planReady({goal:'localFiles'},[])).toBe(true);expect(planReady({goal:'exportPackage'},[])).toBe(true);
  expect(planReady({goal:'externalDraft'},[])).toBe(false);expect(planReady({goal:'externalDraft',profileId:p.id,profileRevision:2},[p])).toBe(true);
  expect(planReady({goal:'externalDraft',profileId:p.id,profileRevision:1},[p])).toBe(false);expect(planReady({goal:'scheduled',profileId:p.id,profileRevision:2},[p])).toBe(false);
  expect(planReady({goal:'localFiles',profileId:p.id,profileRevision:2},[p])).toBe(false);
 });
 it('does not transfer a mock check to a later setting revision',()=>{
  const checks=[{profileId:p.id,profileRevision:1,status:'PASS'},{profileId:p.id,profileRevision:2,status:'UNKNOWN'},{profileId:'other',profileRevision:2,status:'PASS'}] as MockCheck[];
  expect(currentMock(p,checks).map(c=>c.status)).toEqual(['UNKNOWN']);expect(checks).toHaveLength(3);
 });
 it('keeps file output available without connections and explains external goal limits',()=>{
  const html=renderToStaticMarkup(createElement(CompletionSelector,{value:{goal:'exportPackage'},onChange:()=>{},profiles:[],disabled:false}));
  expect(html).toContain('外部ツールへの接続は不要');expect(html).toContain('引き渡し用ファイルの出力まで');
  const external=renderToStaticMarkup(createElement(CompletionSelector,{value:{goal:'externalDraft',profileId:p.id,profileRevision:2},onChange:()=>{},profiles:[p],disabled:false}));expect(external).toContain('外部操作は未完了として残ります');
 });
 it('renders configuration truth without a token/password input or network action',()=>{
  const client:CoreClient={environment:'preview',getSnapshot:async()=>{throw Error('unexpected')},subscribe:async()=>()=>{},execute:async()=>{throw Error('unexpected external operation')}};
  const html=renderToStaticMarkup(createElement(ConnectionSettings,{client,onProfiles:()=>{}}));
  expect(html).toContain('模擬試験の成功は、実接続の成功を示しません');expect(html).toContain('通信せず模擬試験を実行');expect(html).not.toContain('type="password"');expect(html).not.toContain('name="apiKey"');
 });
});
