import {coreLabel,t} from '../i18n';
import {builtInPeople} from './stage-assets';
import type {PresentationSnapshot,PresentationStatus,PresentationTask,RoleDefinition} from '../core/presentation';

export interface Station {readonly role:RoleDefinition;readonly task:PresentationTask|null;readonly label:string;readonly sprite:string|null;readonly prop:string;readonly status:PresentationStatus;readonly moving:boolean}
export function stationsFor(s:PresentationSnapshot,visible=true):Station[]{
 return s.roles.map(role=>{
  // Preserve all historical attempts in Core; display only the highest recorded attempt.
  const tasks=s.tasks.filter(t=>t.roleId===role.id);
  const latestByNode=new Map<string,PresentationTask>();
  for(const t of tasks){const old=latestByNode.get(t.nodeId);if(!old||t.attempt>=old.attempt)latestByNode.set(t.nodeId,t);}
  const task=s.roleMapVersion==='social-text-roles-v2'?(tasks.filter(t=>latestByNode.get(t.nodeId)===t).at(-1)??null):tasks.reduce<PresentationTask|null>((last,t)=>!last||t.attempt>last.attempt?t:last,null);
  const visual=builtInPeople[role.id];
  const status=task?.status??'IDLE';
  return {role,task,label:visual?coreLabel('role',role.id):role.id,sprite:visual?.sprite??null,prop:visual?.prop??'paper',status,
   moving:visible&&s.liveEvidence==='CURRENT'&&s.jobStatus==='RUNNING'&&status==='RUNNING'&&role.actor!=='human'};
 });
}
export function spriteRow(status:PresentationStatus){return status==='RUNNING'?1:status==='AWAITING_APPROVAL'?2:status==='SUCCEEDED'?3:status==='FAILED'?4:status==='UNKNOWN'?5:0;}
export function stationStatus(station:Station){
 const legacy=station.task?.legacyStatus;
 return station.status==='IDLE'&&legacy==='paused'?coreLabel('state','paused'):station.status==='IDLE'&&legacy==='cancelled'?t('presentation.stopped'):coreLabel('presentation',station.status);
}
export function sceneLayout(width:number,count:number){
 const scale=width<800?1:2;
 let columns=Math.max(1,Math.min(count||1,Math.floor(width/(scale===1?120:160))||1));
 if(count>columns&&count%columns===1&&columns>2)columns--;
 const cellWidth=width/columns,rows=Math.max(1,Math.ceil(count/columns)),rowHeight=scale===1?270:340;
 return {columns,cellWidth,scale,rowHeight,height:rows*rowHeight,positions:Array.from({length:count},(_,i)=>({x:(i%columns+.5)*cellWidth,y:(Math.floor(i/columns)+1)*rowHeight-24}))};
}
/** Lines represent recorded verified receipts, never inferred array adjacency. */
export function connectionsFor(s:PresentationSnapshot,stations:readonly Station[]){
 const result=new Map<string,{id:string;from:number;to:number}>();
 for(const e of s.events){
  if(e.status!=='HANDOFF'||e.handoff?.verification!=='FILES_VERIFIED')continue;
  const h=e.handoff,fromTask=s.tasks.find(t=>t.taskId===h.fromTaskId),toTask=s.tasks.find(t=>t.taskId===h.toTaskId);
  const from=stations.findIndex(v=>v.role.id===fromTask?.roleId),to=stations.findIndex(v=>v.role.id===toTask?.roleId);
  if(from>=0&&to>=0&&from!==to)result.set(h.edgeId,{id:h.edgeId,from,to});
 }
 return [...result.values()];
}
export function connectionPath(from:{x:number;y:number},to:{x:number;y:number},width:number){
 const railFrom=from.y+14,railTo=to.y+14;
 return from.y===to.y?`M ${from.x} ${from.y} V ${railFrom} H ${to.x} V ${to.y}`:
  `M ${from.x} ${from.y} V ${railFrom} H ${width-8} V ${railTo} H ${to.x} V ${to.y}`;
}
