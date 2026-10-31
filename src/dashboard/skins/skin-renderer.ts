import type {RenderSkin,SkinCharacter} from '../../core/skins';
import type {Station} from '../stage-visuals';
import {spriteRow} from '../stage-visuals';
import type {ActorVisual} from '../StageActor';
export interface SkinGroup {readonly id:string;readonly character:SkinCharacter|null;readonly members:readonly Station[];readonly station:Station}
/** Resolve each role independently, even when a skin assigns one identity to several roles. */
export function compactSkinVisual(skin:RenderSkin,station:Station,scale:number):ActorVisual|undefined{
 const character=skinGroups([station],skin)[0]?.character;if(!character)return;
 const asset=skinAsset(skin,character.assetId),spec=asset.spec;if(spec.frameWidth>96||spec.frameHeight>96)throw Error('SKIN_SPRITE');
 const a=skin.animations.animations.find(a=>a.roleId===station.role.id&&a.status===station.status);
 if(a&&(![a.frame,a.row,a.frameCount,a.durationMs,a.iterations].every(Number.isSafeInteger)||a.assetId!==character.assetId||a.row>=spec.rows||a.frame+a.frameCount>spec.columns||a.frameCount<1||a.durationMs<100||a.durationMs>3000||a.iterations<1||a.iterations>8))throw Error('SKIN_ANIMATION');
 return {url:asset.dataUrl,scale,width:spec.frameWidth*scale,height:spec.frameHeight*scale,columns:spec.columns,rows:spec.rows,frame:a?.frame??0,row:a?.row??Math.min(spriteRow(station.status),spec.rows-1),count:station.moving?(a?.frameCount??1):1,duration:a?.durationMs??900,iterations:a?.iterations??1,prop:character.prop,background:skin.layout.backgroundAssetId?skinAsset(skin,skin.layout.backgroundAssetId).dataUrl:undefined};
}
/** Groups visual identities only. All original Role/actor/task/status remain visible. */
export function skinGroups(stations:readonly Station[],skin:RenderSkin):SkinGroup[]{
 if(skin.layout.schemaVersion!==1||skin.layout.characters.length>32||skin.layout.assignments.length>64)throw Error('SKIN_LAYOUT');
 const groups=new Map<string,{character:SkinCharacter|null;members:Station[]}>();
 const inheritedRoles:Readonly<Record<string,string>>={researcher:'planner',director:'reviewer',creator:'writer',inspector:'verifier'};
 for(const station of stations){const assignment=skin.layout.assignments.find(a=>a.roleId===station.role.id)??skin.layout.assignments.find(a=>a.roleId===inheritedRoles[station.role.id]);const character=skin.layout.characters.find(c=>c.characterId===assignment?.characterId)??null;
  const id=Object.hasOwn(inheritedRoles,station.role.id)?`role:${station.role.id}`:character?`character:${character.characterId}`:`role:${station.role.id}`;if(!groups.has(id))groups.set(id,{character,members:[]});groups.get(id)!.members.push(station);
 }
 return [...groups].map(([id,g])=>({...g,id,station:g.members.find(s=>s.moving)??g.members.find(s=>s.status==='UNKNOWN')??g.members.find(s=>s.status==='AWAITING_APPROVAL')??g.members[0]}));
}
export function skinAsset(skin:RenderSkin,id:string){const a=skin.assets[id];if(!a||!/^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(a.dataUrl)||a.dataUrl.length>12*1024*1024)throw Error('SKIN_ASSET');const s=a.spec;if(![s.width,s.height,s.frameWidth,s.frameHeight,s.columns,s.rows].every(v=>Number.isSafeInteger(v)&&v>0)||s.width>4096||s.height>4096||s.columns>8||s.rows>8||s.frameWidth*s.columns!==s.width||s.frameHeight*s.rows!==s.height)throw Error('SKIN_DIMENSIONS');return a;}
