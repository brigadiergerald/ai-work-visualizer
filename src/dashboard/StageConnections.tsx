import {t as tr} from '../i18n';
import type {PresentationSnapshot,PresentationEvent} from '../core/presentation';
import {connectionsFor,connectionPath,sceneLayout,type Station} from './stage-visuals';
export default function StageConnections({snapshot,stations,width,handoffs,rowHeight}:{snapshot:PresentationSnapshot;stations:readonly Station[];width:number;handoffs:readonly PresentationEvent[];rowHeight?:number}){
 const base=sceneLayout(width,stations.length),height=Math.max(base.rowHeight,rowHeight??0),layout={...base,height:Math.max(1,Math.ceil(stations.length/base.columns))*height,positions:base.positions.map((p,i)=>({...p,y:(Math.floor(i/base.columns)+1)*height-24}))},connections=connectionsFor(snapshot,stations);
 return <svg className="stage-connections" viewBox={`0 0 ${width} ${layout.height}`} width={width} height={layout.height} aria-label={tr("stageConnections.001")} role="img">
  {connections.map(c=>{const d=connectionPath(layout.positions[c.from],layout.positions[c.to],width);return <path key={c.id} d={d} fill="none"/>;})}
  {snapshot.liveEvidence==='CURRENT'&&handoffs.map(event=>{
   const edge=connections.find(c=>c.id===event.handoff?.edgeId);if(!edge)return null;
   const d=connectionPath(layout.positions[edge.from],layout.positions[edge.to],width);
   return <circle key={event.sourceEventId} className="handoff-spark" r="4" data-receipt={event.sourceEventId}><animateMotion path={d} dur="1.1s" repeatCount="1" fill="freeze"/></circle>;
  })}
 </svg>;
}
