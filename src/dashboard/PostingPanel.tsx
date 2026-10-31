import {useEffect,useRef,useState} from 'react';
import {createPortal} from 'react-dom';
import {t,formatCore,getUiLocale,formatDate,useUiLocale,coreLabel} from '../i18n';
import type {CoreClient,Job} from '../core/contract';
import type {ProviderConnection,ProviderState} from '../core/providers';
import {type ManualPost,type PostingView,type PostingAction,type DeliveryMode,postMode,threadsTargets,tokyoToUtc,maySubmit,postingDate} from '../core/posting';
import ProviderPanel from './ProviderPanel';
import './posting.css';
export default function PostingPanel({client,job,compact=false,workspaceTarget,onShowWorkspace}:{client:CoreClient;job:Job;compact?:boolean;workspaceTarget?:HTMLElement|null;onShowWorkspace?:()=>void}){
 useUiLocale();const [connections,setConnections]=useState<ProviderConnection[]>([]),[posts,setPosts]=useState<ManualPost[]>([]),[selected,setSelected]=useState(''),[time,setTime]=useState(''),[review,setReview]=useState<ManualPost|null>(null),[confirmed,setConfirmed]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState(''),[config,setConfig]=useState(false);
 const pending=useRef(false),epoch=useRef(0),desktop=client.environment==='desktop';
 const [mode,setMode]=useState<DeliveryMode>('scheduled');
 const [platform,setPlatform]=useState('threads');
 const targets=platform==='threads'?threadsTargets(connections):[],chosen=targets.find(r=>`${r.connection.id}:${r.channel.id}`===selected);
 async function load(){const token=epoch.current;const [providers,result]=await Promise.all([client.execute<ProviderState>({action:'providers',request:{operation:'get'}}),client.execute<PostingView>({action:'posting',request:{operation:'get',jobId:job.id}})]);if(token!==epoch.current)return;
  setConnections(providers.connections);if(result.jobId!==job.id)throw Error('POST_WRONG_JOB');setPosts(result.posts);setReview(current=>current?result.posts.find(r=>r.id===current.id)??null:null);
 }
 useEffect(()=>{if(desktop)void load().catch(e=>setError(String(e)));return()=>{++epoch.current;};},[client,job.id]);
 useEffect(()=>{setConfirmed(false);},[review?.id,review?.revision,selected,time,mode,platform]);
 const ready=desktop&&job.state==='completed'&&job.mode==='real'&&job.quality.filesVerified;
 const alreadySubmitted=posts.some(p=>Boolean(p.submissionId));
 const due=tokyoToUtc(time),reviewMatches=review&&chosen&&postMode(review)===mode&&review.connectionId===chosen?.connection.id&&review.connectionRevision===chosen.connection.revision&&review.channelId===chosen.channel.id&&(mode==='immediate'||due===review.dueAt);
 const timing=(p:ManualPost)=>postMode(p)==='immediate'?t('post.nowTiming'):`${postingDate(p.dueAt,getUiLocale())} · Asia/Tokyo`;
 async function run(request:PostingAction){if(pending.current)return;pending.current=true;setBusy(true);setError('');const token=epoch.current;
  try{const result=await client.execute<{post:ManualPost}>({action:'posting',request});if(token!==epoch.current)return;if(result.post.jobId!==job.id)throw Error('POST_WRONG_JOB');setReview(result.post);setConfirmed(false);await load();}
  catch(e){if(token===epoch.current){setError(String(e));try{await load();}catch{/* Never repeat a mutation on an uncertain response. */}}}
  finally{pending.current=false;if(token===epoch.current)setBusy(false);}
 }
 const explain=/TIME_/.test(error)?t('post.timeError'):/STALE|CHANGED|EXPIRED|USED/.test(error)?t('post.staleError'):/ALREADY/.test(error)?t('post.duplicateError'):/TARGET|ACCOUNT/.test(error)?t('post.targetError'):t('post.failure');
 const fullPanel=<section id="manual-posting" className="posting-panel panel" aria-labelledby={`posting-${job.id}`}>
  <h3 id={`posting-${job.id}`}>{t('post.title')}</h3><p className="muted">{t('post.scope')}</p>
  {error&&<p role="alert" className="error">{explain}<br/><code>{error}</code></p>}
  {!ready&&<p>{t('post.notReady')}</p>}
  {alreadySubmitted&&<p className="hint">{t('post.alreadySubmitted')}</p>}
  {platform!=='threads'&&<p role="status">{t('workbench.unsupported')}</p>}
  <details open={config} onToggle={e=>setConfig(e.currentTarget.open)}><summary>{t('post.connectionSettings')}</summary>{config&&<ProviderPanel client={client} onChanged={()=>void load().catch(e=>setError(String(e)))}/>}</details>
  <div className="action-row"><button disabled={busy||!desktop} onClick={()=>void load().catch(e=>setError(String(e)))}>{t('post.refreshConnections')}</button></div>
  {!targets.length&&<p className="hint">{t('post.noTarget')}</p>}
  <fieldset disabled={busy||!ready||alreadySubmitted}><div className="action-row" role="group" aria-label={t('post.mode')}><button type="button" aria-pressed={mode==='scheduled'} onClick={()=>setMode('scheduled')}>{t('post.scheduledMode')}</button><button type="button" aria-pressed={mode==='immediate'} onClick={()=>setMode('immediate')}>{t('post.immediateMode')}</button></div><label>{t('post.destination')}<select aria-label={t('post.destination')} value={selected} onChange={e=>setSelected(e.target.value)}><option value="">{t('post.choose')}</option>{targets.map(({connection:c,channel:ch})=><option key={`${c.id}:${ch.id}`} value={`${c.id}:${ch.id}`}>Threads · @{ch.name} · {c.id.slice(0,8)}</option>)}</select></label>
  {chosen&&<p className="hint">{t('post.accountId')}: <code>{chosen.connection.accountId}</code><br/>{t('post.channelId')}: <code>{chosen.channel.id}</code></p>}
  {mode==='scheduled'?<label>{t('post.date')}<input aria-label={t('post.date')} type="datetime-local" step="60" value={time} onChange={e=>setTime(e.target.value)}/><small>{t('post.timeHelp')}</small></label>:<p className="hint">{t('post.nowHelp')}</p>}
  <button disabled={!chosen||(mode==='scheduled'&&!due)} onClick={()=>{if(!chosen)return;const binding={jobId:job.id,connectionId:chosen.connection.id,connectionRevision:chosen.connection.revision,channelId:chosen.channel.id};if(mode==='immediate')void run({operation:'prepareNow',...binding});else if(due)void run({operation:'prepare',...binding,dueAt:due});}}>{t(mode==='immediate'?'post.reviewNow':'post.review')}</button></fieldset>
  {review&&<article className="posting-review" aria-label={t('post.reviewTitle')}><h4>{t('post.reviewTitle')}</h4><p><strong>Threads · @{review.channelName}</strong><br/>{timing(review)}</p><pre className="posting-text">{review.text}</pre><p className="hint">{t('post.length',{count:[...review.text].length})}<br/>SHA-256 <code>{review.textSha256}</code></p>
   <p className="posting-status" role="status">{formatCore(getUiLocale(),`post.state.${review.state}`)}</p>
   {review.state==='REVIEW_REQUIRED'&&<><label className="posting-confirm"><input type="checkbox" checked={confirmed} disabled={busy||!reviewMatches||alreadySubmitted} onChange={e=>setConfirmed(e.target.checked)}/>{t(postMode(review)==='immediate'?'post.confirmNow':'post.confirm')}</label>{!reviewMatches&&<p>{t('post.changedForm')}</p>}<button className="primary" disabled={busy||!ready||alreadySubmitted||!reviewMatches||!maySubmit(review,job.id,confirmed)} onClick={()=>void run({operation:'submit',recordId:review.id,revision:review.revision,reviewToken:review.reviewToken})}>{t(postMode(review)==='immediate'?'post.submitNow':'post.submit')}</button><p className="hint">{t(postMode(review)==='immediate'?'post.realEffectNow':'post.realEffect')}</p></>}
  </article>}
  {posts.length>0&&<div className="posting-history"><h4>{t('post.history')}</h4>{posts.filter(p=>p.state!=='REVIEW_REQUIRED').map(p=><article key={p.id}><strong>Threads · @{p.channelName}</strong><p>{timing(p)}<br/>{formatCore(getUiLocale(),`post.state.${p.state}`)}</p>{p.error&&<p className="error"><code>{p.error}</code></p>}<p className="hint">{t('post.receipt')}: <code>{p.receiptId??'UNKNOWN'}</code><br/>{t('post.providerStatus')}: {p.providerStatus??'UNKNOWN'}</p><button disabled={busy||!desktop} onClick={()=>void run({operation:'check',recordId:p.id,revision:p.revision})}>{t('post.check')}</button>{p.externalLink&&<p><a href={p.externalLink} target="_blank" rel="noreferrer">{t('post.externalLink')}</a><small>{t('post.externalNotVerified')}</small></p>}<details><summary>{t('post.evidence')}</summary><pre className="posting-text">{p.text}</pre><p>Job <code>{p.jobId}</code><br/>{t('post.accountId')} <code>{p.accountId}</code><br/>{t('post.channelId')} <code>{p.channelId}</code><br/>SHA-256 <code>{p.textSha256}</code></p><ul>{p.history.map((h,i)=><li key={i}>{formatDate(h.at)} · {h.event} · {h.state}{h.error&&` · ${h.error}`}</li>)}</ul></details></article>)}</div>}
 </section>;
 if(!compact)return fullPanel;
 return <section className="posting-dock" aria-label={t('workbench.connectionDetails')}>
  <div className="posting-dock-fields">
   <label>{t('workbench.platform')}<select aria-label={t('workbench.platform')} value={platform} disabled={busy} onChange={e=>{setPlatform(e.target.value);setSelected('');}}><option value="threads">Threads</option><option value="x">X</option></select></label>
   <label>{t('workbench.account')}<select aria-label={t('workbench.account')} value={selected} disabled={busy||!targets.length} onChange={e=>setSelected(e.target.value)}><option value="">{t('workbench.unselected')}</option>{targets.map(({connection:c,channel:ch})=><option key={`${c.id}:${ch.id}`} value={`${c.id}:${ch.id}`}>@{ch.name} · {c.accountId??'UNKNOWN'}</option>)}</select></label>
   <p className="posting-dock-connection" role="status">{t('workbench.connection')}<strong>{platform!=='threads'?t('workbench.unsupported'):chosen?coreLabel('provider',chosen.connection.state):t('workbench.unverified')}</strong>{chosen&&<small>{t('post.accountId')}: {chosen.connection.accountId??'UNKNOWN'}</small>}{error&&<small className="error">{explain}</small>}</p>
  </div>
  <div className="posting-dock-actions"><button disabled={busy||!desktop} onClick={()=>{setConfig(true);onShowWorkspace?.();}}>{t('dashboard.025')}</button><button className="primary" disabled={busy||!ready||platform!=='threads'} onClick={()=>onShowWorkspace?.()}>{t('workbench.posting')}</button></div>
  {workspaceTarget&&createPortal(fullPanel,workspaceTarget)}
 </section>;
}
