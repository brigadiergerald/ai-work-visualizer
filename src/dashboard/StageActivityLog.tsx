import {formatDate,t as tr} from '../i18n';
import {Clock3,Check,TriangleAlert,HelpCircle,Pause,FileText} from 'lucide-react';
import type {Job} from '../core/contract';
export default function StageActivityLog({logs,expanded=false}:{readonly logs:Job['logs'];readonly expanded?:boolean}){
 const latest=logs.at(-1);
 return <details className="workshop-log" open={expanded||undefined}><summary className="workshop-log-caption"><FileText size={16} aria-hidden="true"/><span>{tr("stageActivityLog.001")}</span><span className="log-count">{tr("count.events",{count:logs.length})}</span><span className="log-latest">{latest?.message??tr("stageActivityLog.004")}</span><small>{tr("stageActivityLog.002")}</small></summary><div className="logs" role="log" aria-label={tr("stageActivityLog.005")} aria-live="polite"><p className="log-source">{tr("stageActivityLog.003")}</p>{logs.length?logs.map(log=>{
  const Icon=log.state==='completed'?Check:log.state==='failed'?TriangleAlert:log.state==='unknown'?HelpCircle:log.state==='awaiting_approval'?Pause:Clock3;
  return <div className={`log state-${log.state}`} key={log.id}><time>{formatDate(log.at)}</time><Icon size={15} aria-hidden="true"/><p>{log.message}</p></div>;
 }):<p className="empty">{tr("stageActivityLog.004")}</p>}</div></details>;
}
