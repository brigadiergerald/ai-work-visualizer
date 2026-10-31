import {afterEach,expect,it} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import {CoreUiError,catalogs,coreLabel,coreText,errorMessage,formatCore,formatDate,getUiLocale,localizedSkinText,message,normalizeLocale,persistLocale,presentationKey,readLocale,setUiLocale,t,uiMessage} from '../src/i18n';
import StageScene from '../src/dashboard/StageScene';
import StageActivityLog from '../src/dashboard/StageActivityLog';
import SocialTextForm from '../src/dashboard/purposes/SocialTextForm';
import {stationsFor} from '../src/dashboard/stage-visuals';
import {skinText,type RenderSkin,skinProblemCode} from '../src/core/skins';
import {initialSocialInput,socialInputHash,socialInputIssueCode} from '../src/core/purposes';
import type {PresentationSnapshot} from '../src/core/presentation';
import source from '../tests/fixtures/presentation-awaiting.json';
import render from '../tests/fixtures/render-skin.json';
afterEach(()=>setUiLocale('ja'));
const memory=(initial:Record<string,string>={})=>{const data={...initial};return {data,getItem:(k:string)=>data[k]??null,setItem:(k:string,v:string)=>{data[k]=v;}};};
it('has identical nonempty Core keys and interpolation contracts in both catalogs',()=>{
 expect(Object.keys(catalogs.en).sort()).toEqual(Object.keys(catalogs.ja).sort());
 for(const key of Object.keys(catalogs.ja) as (keyof typeof catalogs.ja)[]){
  const slots=(s:string)=>[...s.matchAll(/\{(\w+)\}/g)].map(m=>m[1]).sort();
  expect(catalogs.ja[key].trim(),key).not.toBe('');expect(catalogs.en[key].trim(),key).not.toBe('');
  expect(slots(catalogs.en[key]),key).toEqual(slots(catalogs.ja[key]));
 }
});
it('retains distinct safety meanings for approval, stop, failure, UNKNOWN, permission and recovery',()=>{
 for(const locale of ['ja','en'] as const){setUiLocale(locale);
  const keys=['state.failed','state.unknown','state.awaiting_approval','state.cancelled','provider.FORBIDDEN','dashboard.040','dashboard.095','dashboard.098','dashboard.099'] as const;
  const values=keys.map(k=>t(k));expect(new Set(values).size).toBe(keys.length);
  expect(t('state.unknown')).toContain('UNKNOWN');expect(t('browserPanel.028')).toContain('NOT_GRANTED');
 }
});
it('falls back to trusted Core Japanese for missing, empty or parameter-incompatible translations',()=>{
 expect(formatCore('en','state.failed',{},{})).toBe('FAILED');
 expect(formatCore('en','state.failed',{}, {'state.failed':''})).toBe(catalogs.ja['state.failed']);
 expect(formatCore('en','count.events',{count:2},{'count.events':'{wrong}'})).toBe('2件');
 expect(formatCore('en','nonexistent')).toContain('UNKNOWN');expect(formatCore('en','toString')).toContain('UNKNOWN');
 expect(formatCore('en','count.events')).toContain('UNKNOWN');
});
it('interpolates values as literal text and never reinterprets supplied braces',()=>{
 expect(t('count.approvals',{count:3})).toBe('承認 3件');
 setUiLocale('en');expect(t('browserPanel.050',{p0:'{p1}',p1:2})).toBe('{p1} account 2');
});
it('defaults to Japanese for absent, corrupt and unsupported presentation preferences',()=>{
 for(const raw of [undefined,'broken','null','[]','7','{"uiLocale":"de"}','{"uiLocale":{}}']){
  const store=memory(raw===undefined?{}:{[presentationKey]:raw});const before=JSON.stringify(store.data);
  expect(readLocale(store)).toBe('ja');expect(JSON.stringify(store.data)).toBe(before);
 }
 expect(normalizeLocale('EN')).toBe('ja');expect(readLocale(undefined)).toBe('ja');
});
it('persists only uiLocale while retaining old skin and content settings, and restores from a fresh reader',()=>{
 const store=memory({[presentationKey]:JSON.stringify({skin:'blue',unknownField:{value:1}}),'ai-guild.dashboard.theme.v1':'free-mint-test','skin-selection-v1':'original','contentLanguage':'ja'});
 const before={...store.data};expect(persistLocale(store,'en')).toBe(true);expect(readLocale(store)).toBe('en');
 expect(JSON.parse(store.data[presentationKey])).toEqual({skin:'blue',unknownField:{value:1},uiLocale:'en'});
 for(const key of Object.keys(before).filter(k=>k!==presentationKey))expect(store.data[key]).toBe(before[key]);
 expect(persistLocale(store,'ja')).toBe(true);expect(readLocale(store)).toBe('ja');
});
it('reports unavailable storage without throwing or claiming persistence',()=>{
 expect(readLocale({getItem:()=>{throw Error('denied');}})).toBe('ja');
 expect(persistLocale({getItem:()=>null,setItem:()=>{throw Error('quota');}},'en')).toBe(false);
});
it('relocalizes UI-owned queued notices but preserves opaque service errors and logs verbatim',()=>{
 const notice=message('skin.selected'),raw='保存済み原文 UNKNOWN FAILED / <script> / {count}';setUiLocale('en');
 expect(uiMessage(notice)).toBe(catalogs.en['skin.selected']);expect(uiMessage(raw)).toBe(raw);
 expect(uiMessage(errorMessage(new CoreUiError('dashboard.068')))).toBe(catalogs.en['dashboard.068']);
 expect(coreText('providerError','TIMEOUT',raw)).toContain('UNKNOWN');expect(coreText('providerError','unstructured',raw)).toBe(raw);
 const markup=renderToStaticMarkup(<StageActivityLog logs={[{id:'1',at:'2026-10-30T00:00:00Z',stageId:null,state:'failed',message:raw,code:'unstructured'}]}/>);
 expect(markup).toContain('Activity log');expect(markup).toContain('保存済み原文 UNKNOWN FAILED');expect(markup).toContain('&lt;script&gt;');
});
it('selects requested skin metadata with stable fallback and cannot override Core safety copy',()=>{
 const skin=structuredClone(render) as RenderSkin;
 (skin.locales.en.strings as Record<string,string>)['state.failed']='Success';setUiLocale('en');
 expect(localizedSkinText({ja:'日本語名',en:'English name'},'Paper')).toBe('English name');
 expect(localizedSkinText({ja:'日本語名'},'Paper')).toBe('日本語名');expect(localizedSkinText({},'Paper')).toBe('Paper');
 expect(skinText(skin,'state.failed','en','FAILED')).toBe('FAILED');expect(t('state.failed')).toBe('FAILED');
});
it('maps explicit skin error codes and never infers successful or conflicting results from prose',()=>{
 expect(skinProblemCode('SKIN_VERSION_CONFLICT')).toBe('skin.conflict');expect(skinProblemCode('Error: SKIN_ARCHIVE_SIZE')).toBe('skin.limit');
 expect(skinProblemCode('a prose CONFLICT with VERSION')).toBe('skin.invalid');
});
it('changes display without modifying event, role, attempts, input hashes or motion eligibility',async()=>{
 const s=structuredClone(source) as PresentationSnapshot,before=JSON.stringify(s),input={...initialSocialInput(),theme:'固定本文\nExact text'},hash=await socialInputHash(input);
 setUiLocale('ja');const identities=stationsFor(s).map(({role,task,moving})=>({role,task,moving}));
 for(const locale of ['en','ja','en'] as const){setUiLocale(locale);expect(stationsFor(s).map(({role,task,moving})=>({role,task,moving}))).toEqual(identities);expect(JSON.stringify(s)).toBe(before);expect(await socialInputHash(input)).toBe(hash);expect(input.contentLanguage).toBe('ja');}
 expect(getUiLocale()).toBe('en');
});
it('renders both workshop languages with unchanged human stillness and no historical handoff replay',()=>{
 const s=source as PresentationSnapshot;
 for(const locale of ['ja','en'] as const){setUiLocale(locale);const html=renderToStaticMarkup(<StageScene snapshot={s} handoffs={[]} hasJob visible/>);
  expect(html).not.toContain(t('stageScene.001'));expect(html).toContain(coreLabel('actor','human').replaceAll('&','&amp;'));expect(html).not.toContain('<animateMotion');expect(stationsFor(s).find(v=>v.role.actor==='human')?.moving).toBe(false);
 }
});
it('uses independent content-language values and localized validation labels in the actual social form',()=>{
 setUiLocale('en');const value={...initialSocialInput(),theme:'変更しない',contentLanguage:'ja' as const};
 const html=renderToStaticMarkup(<SocialTextForm value={value} onChange={()=>{throw Error('render must not mutate');}} definition={undefined} completionPlan={{goal:'localFiles'}} onPlan={()=>{}} profiles={[]} busy={false} locked={false} canStart={false} startBlockedReason="" onStart={()=>{}}/>);
 expect(html).toContain('Generated text language');expect(html).toContain('value="ja" selected');expect(html).toContain('変更しない');expect(socialInputIssueCode({...value,theme:''})).toBe('social.themeRequired');
});
it('formats valid dates for the UI locale and preserves invalid source dates',()=>{
 setUiLocale('ja');const j=formatDate('2026-10-30T00:00:00Z');setUiLocale('en');expect(formatDate('2026-10-30T00:00:00Z')).not.toBe(j);expect(formatDate('literal invalid date')).toBe('literal invalid date');
});
// These compile-time checks fail the TypeScript gate if the interpolation contract drifts.
function parameterTypeChecks(){
 // @ts-expect-error missing parameter
 t('count.events');
 // @ts-expect-error incorrect parameter name
 t('count.events',{wrong:1});
 // @ts-expect-error invalid key
 t('skin.external.core.approval');
}
void parameterTypeChecks;
