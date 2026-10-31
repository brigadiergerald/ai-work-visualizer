import {useEffect,useRef,useState,type CSSProperties} from 'react';
import {Radio,RadioOff} from 'lucide-react';
import {t,coreLabel} from '../i18n';
import type {PresentationSnapshot,PresentationEvent} from '../core/presentation';
import type {RenderSkin} from '../core/skins';
import {builtInPeople,spriteSpec} from './stage-assets';
import {stationsFor,type Station} from './stage-visuals';
import StageActor from './StageActor';
import StageConnections from './StageConnections';
import {compactSkinVisual} from './skins/skin-renderer';

export function idleStations(purpose:string):Station[]{
 const roles=purpose==='social-text'?['researcher','director','creator','inspector']:['planner','writer','verifier','reviewer','archivist'];
 return roles.map(id=>({role:{id,purposeId:purpose,labelKey:`roles.${purpose}.${id}`,actor:id==='reviewer'?'human':['inspector','archivist','verifier'].includes(id)?'host':'codex'},task:null,label:coreLabel('role',id),sprite:builtInPeople[id].sprite,prop:builtInPeople[id].prop,status:'IDLE',moving:false}));
}
/** Compact projection only: no inferred events, statuses, receipts or Core actions. */
export default function CompactStageRoom({snapshot,skin,visible,handoffs,hasJob,purpose,roleMapVersion,onSelectRoles}:{snapshot:PresentationSnapshot|null;skin?:RenderSkin|null;visible:boolean;handoffs:readonly PresentationEvent[];hasJob:boolean;purpose:string;roleMapVersion?:string|null;onSelectRoles?:(roles:readonly string[])=>void}){
 const root=useRef<HTMLDivElement>(null),[size,setSize]=useState({width:700,height:110});
 useEffect(()=>{const el=root.current;if(!el)return;const update=()=>setSize({width:el.clientWidth,height:el.clientHeight});const observer=new ResizeObserver(update);observer.observe(el);update();return()=>observer.disconnect();},[]);
 const stations=snapshot?stationsFor(snapshot,visible):idleStations(hasJob&&roleMapVersion!=='social-text-roles-v2'?'legacy':purpose).map(s=>hasJob?{...s,status:'UNKNOWN' as const}:s);
 const scale=!skin&&size.height>=164&&size.width/Math.max(stations.length,1)>=210?2:1;
 const current=snapshot?.liveEvidence==='CURRENT';
 return <div className="workshop compact-workshop" data-live={snapshot?.liveEvidence??'UNKNOWN'}>
  <div className="compact-evidence"><strong>{t('workbench.team')}</strong><span role="status">{hasJob?<>{current?<Radio size={13}/>:<RadioOff size={13}/>} {t('stageScene.014')}{snapshot?.liveEvidence??'UNKNOWN'}</>:t('workbench.idle')}</span></div>
  <div ref={root} className="compact-stage-room" data-custom={!!skin} data-sprite-scale={scale} style={{'--sprite-width':`${spriteSpec.cellWidth*scale}px`,'--sprite-height':`${spriteSpec.cellHeight*scale}px`} as CSSProperties}>
   {snapshot&&current&&<StageConnections snapshot={snapshot} stations={stations} width={size.width} rowHeight={size.height} handoffs={visible?handoffs:[]}/>}
   <div className="compact-station-grid" style={{gridTemplateColumns:`repeat(${stations.length||1},minmax(0,1fr))`}}>{stations.map(station=><StageActor compact key={station.role.id} station={station} visual={skin?compactSkinVisual(skin,station,scale):undefined} onSelect={onSelectRoles?()=>onSelectRoles([station.role.id]):undefined}/>)}</div>
   {!stations.length&&<p className="empty">{t('stageScene.010')}</p>}
  </div>
 </div>;
}
