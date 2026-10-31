import {afterEach,expect,it} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import {tokyoToUtc,threadsTargets,maySubmit,postingDate,postMode,type ManualPost} from '../src/core/posting';
import type {ProviderConnection} from '../src/core/providers';
import type {Job,CoreClient} from '../src/core/contract';
import {setUiLocale} from '../src/i18n';
import PostingPanel from '../src/dashboard/PostingPanel';
afterEach(()=>setUiLocale('ja'));
it('uses explicit Japan time independently of the computer timezone and rejects invalid dates',()=>{
 expect(tokyoToUtc('2026-11-01T09:30')).toBe('2026-11-01T00:30:00.000Z');expect(tokyoToUtc('2026-02-30T12:00')).toBe(null);expect(tokyoToUtc('2026-11-01')).toBe(null);expect(postingDate('2026-11-01T00:30:00Z','ja')).toContain('09:30');
});
it('shows only explicitly selected verified Threads destinations from the same workspace',()=>{
 const c={id:'c',providerId:'buffer',state:'TARGETS_VERIFIED',selectedWorkspaceId:'w',selectedChannelIds:['a','b','c'],channels:[{id:'a',workspaceId:'w',name:'user',platform:'threads'},{id:'b',workspaceId:'w',platform:'instagram'},{id:'c',workspaceId:'another',platform:'threads'}]} as ProviderConnection;
 expect(threadsTargets([c]).map(t=>t.channel.id)).toEqual(['a']);expect(threadsTargets([{...c,state:'UNKNOWN'}])).toEqual([]);expect(threadsTargets([{...c,providerId:'mock'}])).toEqual([]);
});
it('never enables confirmation for another Job, consumed or unknown submissions',()=>{
 const r={jobId:'j',state:'REVIEW_REQUIRED'} as ManualPost;expect(maySubmit(r,'j',true)).toBe(true);expect(maySubmit(r,'j',false)).toBe(false);expect(maySubmit(r,'another',true)).toBe(false);
 for(const state of ['SUBMISSION_STARTED','PROVIDER_ACCEPTED','SCHEDULED','PUBLISHING','PUBLISHED_REPORTED','UNKNOWN','FAILED'] as const)expect(maySubmit({...r,state},'j',true)).toBe(false);
});
it('renders a clear real scheduling boundary in both languages, disabled before delivery',()=>{
 const job={id:'j',state:'running',mode:'real',quality:{filesVerified:false}} as Job;const client={environment:'offline-fixture'} as CoreClient;
 for(const locale of ['ja','en'] as const){setUiLocale(locale);const html=renderToStaticMarkup(<PostingPanel client={client} job={job}/>);expect(html).toContain(locale==='ja'?'Threadsへの投稿':'Post to Threads');expect(html).toContain(locale==='ja'?'今すぐ投稿':'Post now');expect(html).toContain('disabled');expect(html).toContain('Asia/Tokyo');expect(html).not.toContain('この内容を承認して1件予約する');}
});
it('treats records without an explicit delivery mode as legacy scheduled posts',()=>{const old={} as ManualPost;expect(postMode(old)).toBe('scheduled');expect(postMode({...old,deliveryMode:'immediate'})).toBe('immediate');});
