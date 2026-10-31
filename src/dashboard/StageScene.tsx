import {t as tr} from '../i18n';
import {useEffect,useRef,useState,type CSSProperties} from 'react';
import {Radio,RadioOff} from 'lucide-react';
import type {PresentationEvent,PresentationSnapshot} from '../core/presentation';
import {stationsFor,sceneLayout} from './stage-visuals';
import {spriteSpec} from './stage-assets';
import StageActor from './StageActor';
import StageConnections from './StageConnections';
import './stage-scene.css';
import type {RenderSkin} from '../core/skins';
import SkinRoom from './skins/SkinRoom';
import CompactStageRoom from './CompactStageRoom';

export interface StageSceneProps {readonly snapshot:PresentationSnapshot|null;readonly handoffs:readonly PresentationEvent[];readonly visible:boolean;readonly hasJob:boolean;readonly skin?:RenderSkin|null;readonly onSelectRoles?:(roles:readonly string[])=>void;readonly compact?:boolean;readonly purpose?:string;readonly roleMapVersion?:string|null}
/** Display only: no client, actions, execution, approval or arbitrary asset inputs. */
export default function StageScene({snapshot:s,handoffs,visible,hasJob,skin,onSelectRoles,compact=false,purpose='social-text',roleMapVersion}:StageSceneProps){
 const root=useRef<HTMLDivElement>(null),[width,setWidth]=useState(830),[contentHeight,setContentHeight]=useState(0);
 useEffect(()=>{const el=root.current;if(!el)return;const observer=new ResizeObserver(()=>setWidth(Math.max(1,Math.floor(el.clientWidth))));observer.observe(el);setWidth(Math.max(1,Math.floor(el.clientWidth)));return()=>observer.disconnect();},[]);
 useEffect(()=>{const el=root.current;if(!el||skin)return;const actors=Array.from(el.querySelectorAll<HTMLElement>('.stage-station'));if(!actors.length){setContentHeight(0);return;}const measure=()=>setContentHeight(Math.max(...actors.map(actor=>{const last=actor.lastElementChild;return last?Math.ceil(last.getBoundingClientRect().bottom-actor.getBoundingClientRect().top)+64:0;})));const observer=new ResizeObserver(measure);actors.forEach(actor=>Array.from(actor.children).forEach(child=>observer.observe(child)));measure();return()=>observer.disconnect();},[s,skin,width]);
 const stations=s?stationsFor(s,visible):[],base=sceneLayout(width,stations.length),rowHeight=Math.max(base.rowHeight,contentHeight);
 const layout={...base,rowHeight,height:Math.max(1,Math.ceil(stations.length/base.columns))*rowHeight};
 const current=s?.liveEvidence==='CURRENT';
 if(compact)return <CompactStageRoom snapshot={s} handoffs={handoffs} visible={visible} hasJob={hasJob} skin={skin} onSelectRoles={onSelectRoles} purpose={purpose} roleMapVersion={roleMapVersion}/>;
 return <div className="workshop" data-live={s?.liveEvidence??'UNKNOWN'} data-job-status={s?.jobStatus??'UNKNOWN'}>
  <div className="workshop-evidence" role="status">
   <span className={current?'live-current':'live-uncertain'}>{current?<Radio size={15}/>:<RadioOff size={15}/>}{tr("stageScene.014")}{current?'CURRENT':s?.liveEvidence==='RESYNC_REQUIRED'?tr("stageScene.009"):'UNKNOWN'}</span>
  </div>
  {!current&&hasJob&&<p className="workshop-caution">{tr("stageScene.004")}{s?.liveReasonCode&&<code>{s.liveReasonCode}</code>}</p>}
  <div ref={root} className="stage-viewport" tabIndex={0} aria-label={tr("shell.studio")}>{skin&&s&&stations.length>0?<SkinRoom skin={skin} stations={stations} snapshot={s} width={width} handoffs={handoffs} visible={visible} onSelectRoles={onSelectRoles}/>:<div className="workshop-room" data-sprite-scale={layout.scale} style={{minHeight:stations.length?layout.height:250,'--sprite-width':`${spriteSpec.cellWidth*layout.scale}px`,'--sprite-height':`${spriteSpec.cellHeight*layout.scale}px`,'--station-height':`${layout.rowHeight}px`,'--diorama-height':layout.scale===1?'100px':'166px'} as CSSProperties}>
   {s&&stations.length>0?<><StageConnections snapshot={s} stations={stations} width={width} rowHeight={rowHeight} handoffs={visible?handoffs:[]}/><div className="station-grid" style={{gridTemplateColumns:`repeat(${layout.columns}, minmax(0, 1fr))`}}>{stations.map(station=><StageActor key={station.role.id} station={station} onSelect={onSelectRoles?()=>onSelectRoles([station.role.id]):undefined}/>)}</div></>:<div className="workshop-empty"><p>{hasJob?tr("stageScene.010"):tr("refine.ready")}</p></div>}
  </div>}</div>
 </div>;
}
