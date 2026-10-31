import {coreLabel,t as tr} from '../i18n';
import {BookOpen,FileCheck2,FileText,Archive,LampDesk,Check,Pause,TriangleAlert,HelpCircle,Clock3} from 'lucide-react';
import {spriteRow,stationStatus,type Station} from './stage-visuals';
import type {CSSProperties} from 'react';
export interface ActorVisual{readonly url:string;readonly width:number;readonly height:number;readonly scale:number;readonly columns:number;readonly rows:number;readonly frame:number;readonly row:number;readonly count:number;readonly duration:number;readonly iterations:number;readonly prop:string;readonly background?:string}
export default function StageActor({station:s,visual,onSelect,compact=false}:{station:Station;readonly visual?:ActorVisual;readonly onSelect?:()=>void;readonly compact?:boolean}){
 const prop=visual?.prop??s.prop;const Prop=prop==='books'?BookOpen:prop==='cabinet'?Archive:prop==='lamp'?LampDesk:prop==='check'?FileCheck2:FileText;
 const StateIcon=s.status==='SUCCEEDED'?Check:s.status==='FAILED'?TriangleAlert:s.status==='UNKNOWN'?HelpCircle:s.status==='AWAITING_APPROVAL'?Pause:Clock3;
 return <article data-role={s.role.id} className={`stage-station stage-${s.status.toLowerCase()} ${s.moving?'is-working':''}`} aria-label={`${s.label}：${stationStatus(s)}`} style={compact&&visual?{'--sprite-width':`${visual.width}px`,'--sprite-height':`${visual.height}px`} as CSSProperties:undefined}>
  <div className="station-heading"><h3>{onSelect?<button className="station-decision-link" title={tr('decisions.jump')} onClick={onSelect}>{s.label}</button>:s.label}</h3><small>{coreLabel('actor',s.role.actor)}</small></div>
  <div className="station-diorama" aria-hidden="true" style={visual?{height:visual.height+(compact?0:visual.scale===1?28:22)}:undefined}>
   {visual?.background&&<img className="skin-background" alt="" src={visual.background}/>}
   <div className="station-floor"/>
   {visual||s.sprite?<div className="actor-motion" style={visual?{'--sprite-width':`${visual.width}px`,'--sprite-height':`${visual.height}px`,width:visual.width,height:visual.height} as CSSProperties:undefined}>
    {visual?<div className="workshop-sprite skin-sprite" style={{'--sprite-width':`${visual.width}px`,'--sprite-height':`${visual.height}px`,'--skin-start':`${-visual.frame*visual.width}px`,'--skin-end':`${-(visual.frame+visual.count)*visual.width}px`,'--skin-duration':`${visual.duration}ms`,'--skin-count':visual.count,'--skin-iterations':visual.iterations,width:visual.width,height:visual.height,backgroundImage:`url("${visual.url}")`,backgroundSize:`${visual.width*visual.columns}px ${visual.height*visual.rows}px`,backgroundPosition:`${-visual.frame*visual.width}px ${-visual.row*visual.height}px`} as CSSProperties}/>:<div className="workshop-sprite" style={{backgroundImage:`url("${s.sprite}")`,backgroundPositionY:`calc(var(--sprite-height) * -${spriteRow(s.status)})`}}/>}
   </div>:<div className="station-placeholder"><FileText size={38}/></div>}
   <div className={`station-furniture furniture-${prop}`}><span className="desk-top"/><span className="desk-leg leg-left"/><span className="desk-leg leg-right"/><Prop className="desk-prop" size={26} strokeWidth={1.8}/><span className="desk-paper"/></div>
   {s.role.actor==='human'&&<span className="approval-lamp"/>}
  </div>
  <p className="station-state"><StateIcon size={14}/>{stationStatus(s)}</p>
  <small className="station-scope">{s.task&&['researcher','director','creator','inspector'].includes(s.role.id)?coreLabel('socialPhase',s.task.nodeId):s.task?tr("stageActor.001"):tr("stageActor.002")}</small>
  {s.task?.reasonCode&&<small className="station-reason">{s.task.reasonCode}</small>}
 </article>;
}
