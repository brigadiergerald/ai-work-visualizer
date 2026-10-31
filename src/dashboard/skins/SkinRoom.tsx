import {coreLabel,t as tr} from '../../i18n';
import {useEffect,useRef,useState,type CSSProperties} from 'react';
import type {RenderSkin} from '../../core/skins';
import type {PresentationEvent,PresentationSnapshot} from '../../core/presentation';
import {connectionsFor,connectionPath,sceneLayout,spriteRow,stationStatus,type Station} from '../stage-visuals';
import StageActor,{type ActorVisual} from '../StageActor';
import {skinAsset,skinGroups} from './skin-renderer';
import './skins.css';
export default function SkinRoom({skin,stations,snapshot,width,handoffs,visible,onSelectRoles}:{readonly skin:RenderSkin;readonly stations:readonly Station[];readonly snapshot:PresentationSnapshot;readonly width:number;readonly handoffs:readonly PresentationEvent[];readonly visible:boolean;readonly onSelectRoles?:(roles:readonly string[])=>void}){
 const grid=useRef<HTMLDivElement>(null),[measured,setMeasured]=useState(0);
 const groups=skinGroups(stations,skin),base=sceneLayout(width,groups.length),columns=skin.layout.columns?Math.min(base.columns,skin.layout.columns):base.columns;
 useEffect(()=>{const children=grid.current?.querySelectorAll<HTMLElement>('.skin-character-content');if(!children?.length)return;const measure=()=>setMeasured(Math.max(...Array.from(children,c=>Math.ceil(c.getBoundingClientRect().height)))+64);const observer=new ResizeObserver(measure);children.forEach(c=>observer.observe(c));measure();return()=>observer.disconnect();},[skin,stations]);
 const members=Math.max(1,...groups.map(g=>g.members.length)),rowHeight=Math.max(base.rowHeight+members*52,measured);
 const layout={...base,columns,rowHeight,height:Math.ceil(groups.length/columns)*rowHeight,positions:groups.map((_,i)=>({x:(i%columns+.5)*width/columns,y:(Math.floor(i/columns)+1)*rowHeight-24}))};
 const index=(role:string)=>groups.findIndex(g=>g.members.some(m=>m.role.id===role));
 const edges=connectionsFor(snapshot,stations).map(e=>({...e,from:index(stations[e.from].role.id),to:index(stations[e.to].role.id)})).filter(e=>e.from>=0&&e.to>=0&&e.from!==e.to);
 const p=skin.layout.palette;if(Object.values(p).some(v=>!/^#[0-9a-f]{6}$/i.test(v)))throw Error('SKIN_PALETTE');
 const style={'--room':p.room,'--room-floor':p.floor,'--wood':p.wood,'--wood-edge':p.woodEdge,'--mint':p.mint,'--station-height':`${rowHeight}px`,minHeight:layout.height} as CSSProperties;
 const bg=skin.layout.backgroundAssetId?skinAsset(skin,skin.layout.backgroundAssetId).dataUrl:undefined;
 return <div className="workshop-room skin-room" data-skin={skin.selection.skinId} data-sprite-scale={base.scale} style={style}><svg className="stage-connections" viewBox={`0 0 ${width} ${layout.height}`} width={width} height={layout.height} role="img" aria-label={tr("stageConnections.001")}>{edges.map(e=><path key={e.id} d={connectionPath(layout.positions[e.from],layout.positions[e.to],width)} fill="none"/>)}{visible&&snapshot.liveEvidence==='CURRENT'&&handoffs.map(event=>{const edge=edges.find(e=>e.id===event.handoff?.edgeId);return edge?<circle key={event.sourceEventId} className="handoff-spark" r="4" data-receipt={event.sourceEventId}><animateMotion path={connectionPath(layout.positions[edge.from],layout.positions[edge.to],width)} dur="1.1s" repeatCount="1" fill="freeze"/></circle>:null;})}</svg><div ref={grid} className="station-grid" style={{gridTemplateColumns:`repeat(${columns},minmax(0,1fr))`}}>{groups.map(g=>{
  let visual:ActorVisual|undefined;
  if(g.character){const asset=skinAsset(skin,g.character.assetId),spec=asset.spec;if(spec.frameWidth>96||spec.frameHeight>96)throw Error('SKIN_SPRITE');
   const a=skin.animations.animations.find(a=>a.roleId===g.station.role.id&&a.status===g.station.status);const frame=a?.frame??0,row=a?.row??Math.min(spriteRow(g.station.status),spec.rows-1),count=g.station.moving?(a?.frameCount??1):1;
   if(a&&(![a.frame,a.row,a.frameCount,a.durationMs,a.iterations].every(Number.isSafeInteger)||a.assetId!==g.character.assetId||a.row>=spec.rows||a.frame+a.frameCount>spec.columns||a.frameCount<1||a.durationMs<100||a.durationMs>3000||a.iterations<1||a.iterations>8))throw Error('SKIN_ANIMATION');
   visual={url:asset.dataUrl,scale:base.scale,width:spec.frameWidth*base.scale,height:spec.frameHeight*base.scale,columns:spec.columns,rows:spec.rows,frame,row,count,duration:a?.durationMs??900,iterations:a?.iterations??1,prop:g.character.prop,background:bg};
  }
  return <div key={g.id} className="skin-character" data-character={g.id} style={{minHeight:rowHeight}}><div className="skin-character-content"><StageActor station={g.station} visual={visual} onSelect={onSelectRoles?()=>onSelectRoles(g.members.map(m=>m.role.id)):undefined}/><ul className="skin-role-members" aria-label={tr("skinRoom.001")}>{g.members.map(m=><li key={m.role.id} data-role={m.role.id} data-actor={m.role.actor}><strong>{m.label}：{stationStatus(m)}</strong><small>{coreLabel('actor',m.role.actor)} · {m.task?tr("stageActor.001"):tr("stageActor.002")}</small>{m.task?.reasonCode&&<small>{m.task.reasonCode}</small>}</li>)}</ul></div></div>;
 })}</div></div>;
}
