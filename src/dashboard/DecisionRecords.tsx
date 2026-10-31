import {coreLabel,formatCore,formatDate,getUiLocale,t,useUiLocale} from '../i18n';
import type {Job} from '../core/contract';
import type {DecisionKind,DecisionRecord} from '../core/decisions';
import './decision-records.css';
const kinds:DecisionKind[]=['theme','direction','writing','inspection'];
function label(code:string){return formatCore(getUiLocale(),`decisions.${code}`);}
function validTimestamp(value:string|null):value is string{
 if(!value||!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(value)||!Number.isFinite(Date.parse(value)))return false;
 const [year,month,day]=value.slice(0,10).split('-').map(Number);
 return month>=1&&month<=12&&day>=1&&day<=new Date(Date.UTC(year,month,0)).getUTCDate();
}
/** Presentation only. Actions stay in the trusted dashboard and existing Core guards. */
export default function DecisionRecords({job,busy,onOpen}:{job:Job|null;busy:boolean;onOpen:(path:string)=>void}){
 useUiLocale();const view=job?.decisions?.jobId===job?.id?job?.decisions:undefined;
 return <section className="decision-records panel" aria-labelledby="decision-title">
  <div className="section-heading"><h2 id="decision-title">{t('decisions.title')}</h2></div>
  {!job?<p className="empty">{t('decisions.empty')}</p>:<>
   <details className="decision-scope"><summary>{t('refine.recordScope')}</summary><p>{t('decisions.scope')}</p><p className="decision-boundary">{t('decisions.localJob')} <strong>{coreLabel('state',job.state)}</strong> · {view?.humanContentApproval==='PENDING'?t('decisions.approvalPending'):view?.humanContentApproval==='RECORDED_SEPARATELY'?t('decisions.approvalSeparate'):t('decisions.approvalNone')} · {t('decisions.noPublication')}</p></details>
   <div className="decision-list">{kinds.map(kind=>{
    const r=view?.records.find(r=>r.kind===kind&&r.jobId===job.id);
    const recorded=r?.status==='RECORDED'&&r.source?.integrity==='verified';
    const stage=r?.stageState??job.stages.find(s=>s.id===(kind==='theme'||kind==='direction'?'plan':kind==='inspection'?'qa':'writing'))?.state??'unknown';
    const state=stage==='disconnected'?'unknown':stage==='handing_off'?'running':stage;
    const summary=recorded&&r?.note?r.note.summary:recorded&&r?.inspection?t('decisions.inspected'):t('decisions.noRecord');
    const reason=recorded&&r?.note?r.note.reason:recorded&&r?.inspection?t('decisions.inspectionReason'):r?.status==='SOURCE_FAILED'?t('decisions.sourceFailed'):r?.status==='INVALID_RECORD'?t('decisions.invalid'):['queued','running','handing_off','paused','awaiting_approval'].includes(stage)?t('decisions.pending'):t('decisions.noInference');
    const timestamp=recorded&&validTimestamp(r.recordedAt)?r.recordedAt:null;
    const timeStatus=!recorded?t('refine.unrecorded'):!r.recordedAt?t('refine.noTime'):timestamp?null:t('refine.invalidTime');
    return <details key={`${job.id}:${kind}`} id={`decision-${kind}`} className="decision-entry">
     <summary>{timestamp?<time dateTime={timestamp} title={formatDate(timestamp)}>{new Date(timestamp).toLocaleTimeString(getUiLocale()==='ja'?'ja-JP':'en-GB',{hour:'2-digit',minute:'2-digit',hour12:false})}</time>:<span className="decision-time-missing">{timeStatus}</span>}<strong>{label(kind)}</strong><small>{r?.status==='SOURCE_FAILED'?t('refine.sourceFailed'):r?.status==='INVALID_RECORD'?t('refine.invalid'):coreLabel('state',state)}</small></summary>
     <div className="decision-detail">
      <p className="decision-summary">{summary}</p><p className="decision-reason">{reason}</p>
      {r?.stageError&&<p className="error">{r.stageError}</p>}
      {r?.sourceFields&&<details><summary>{t('decisions.savedConditions')}</summary><ul>{[...(r.sourceFields.requirements??[]),...(r.sourceFields.productionConstraints??[])].map((s,i)=><li key={i}>{s}</li>)}</ul></details>}
      {recorded&&r?.note&&<ul>{r.note.details.map((s,i)=><li key={i}>{s}</li>)}</ul>}
      {recorded&&r?.inspection&&<Inspection record={r}/>}
      {r&&<dl className="decision-meta"><dt>Job</dt><dd><code>{r.jobId}</code></dd><dt>Task / attempt</dt><dd><code>{r.taskId??'UNKNOWN'} / {r.attempt??'UNKNOWN'}</code></dd><dt>Role / {t('decisions.phase')}</dt><dd>{r.roleId?coreLabel('role',r.roleId):'UNKNOWN'} / {r.nodeId}</dd><dt>{t('decisions.recordedAt')}</dt><dd>{r.recordedAt?formatDate(r.recordedAt):t('decisions.noRecord')}</dd></dl>}
      {r?.source&&<div className="decision-source"><strong>{t('decisions.evidence')}: {r.source.name}</strong><code>{r.source.path}</code><small>SHA-256 <code>{r.source.sha256}</code></small><small>{r.source.bytes.toLocaleString()} bytes · {r.source.integrity==='verified'?t('decisions.fileVerified'):t('decisions.sourceFailed')}</small><button disabled={busy||job.state!=='completed'||r.source.integrity!=='verified'} onClick={()=>onOpen(r.source!.path)}>{t('decisions.openSource')}</button>{job.state!=='completed'&&<small>{t('decisions.openAfterDelivery')}</small>}</div>}
      {!recorded&&<p>{t('decisions.noInference')}</p>}
      {r?.note&&<small>{t('decisions.noteScope')}</small>}
     </div>
    </details>;
   })}</div>
  </>}
 </section>;
}
function Inspection({record:r}:{record:DecisionRecord}){
 const q=r.inspection!;
 return <><h3>{t('decisions.criteriaResults')}</h3><table><thead><tr><th>{t('decisions.criterion')}</th><th>{t('decisions.result')}</th><th>{t('decisions.observed')}</th></tr></thead><tbody>{q.checks.map(c=><tr key={c.criterion}><td>{label(c.criterion)}</td><td>{label(c.result)}</td><td>{c.observed??'UNKNOWN'}</td></tr>)}</tbody></table><h3>{t('decisions.unverified')}</h3><ul>{q.unverified.map(c=><li key={c}>{label(c)}</li>)}</ul><p>{t('decisions.inspectionLimit')}</p><p>{t('decisions.nextStage')}</p></>;
}
