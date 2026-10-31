import standard from './standard.json';
import alternate from './alternate.json';
import type {State} from '../core/contract';
export type Section='compose'|'stages'|'artifacts'|'logs';
export interface Theme {schemaVersion:1;id:string;version:string;name:string;colors:Record<'background'|'surface'|'text'|'muted'|'border'|'accent'|'onAccent',string>;font:'system'|'system-serif';background:'solid'|'grid';layout:{variant:'split'|'stacked';sections:Section[]};assets:{icon:'document'|'leaf';character:null};animations:Record<State,'none'|'pulse'|'outline'>}
const states:State[]=['queued','running','awaiting_approval','paused','completed','failed','cancelled','unknown'];
const sections:Section[]=['compose','stages','artifacts','logs'];
function keys(v:unknown,expected:string[]):asserts v is Record<string,unknown>{if(!v||typeof v!=='object'||Array.isArray(v)||Object.keys(v).length!==expected.length||Object.keys(v).some(k=>!expected.includes(k)))throw new Error('テーマの項目・形式が不正です');}
/** Only finite, inert values. No URL, CSS text, script, markup or remote fonts. */
export function loadTheme(input:unknown):Theme{
 keys(input,['schemaVersion','id','version','name','colors','font','background','layout','assets','animations']);
 if(input.schemaVersion!==1||typeof input.id!=='string'||!/^[a-z][a-z0-9-]{0,63}$/.test(input.id)||typeof input.version!=='string'||!/^\d+\.\d+\.\d+$/.test(input.version)||typeof input.name!=='string'||!input.name.trim()||input.name.length>80)throw new Error('テーマの識別子・版が不正です');
 keys(input.colors,['background','surface','text','muted','border','accent','onAccent']);
 if(Object.values(input.colors).some(c=>typeof c!=='string'||!/^#[\da-f]{6}$/i.test(c)))throw new Error('配色は6桁の色だけ指定できます');
 if(!['system','system-serif'].includes(input.font as string)||!['solid','grid'].includes(input.background as string))throw new Error('フォント・背景が不正です');
 keys(input.layout,['variant','sections']);
 if(!['split','stacked'].includes(input.layout.variant as string)||!Array.isArray(input.layout.sections)||input.layout.sections.length!==4||new Set(input.layout.sections).size!==4||input.layout.sections.some(s=>!sections.includes(s)))throw new Error('レイアウトが不正です');
 keys(input.assets,['icon','character']);if(!['document','leaf'].includes(input.assets.icon as string)||input.assets.character!==null)throw new Error('未登録素材は読み込めません');
 keys(input.animations,states);if(Object.values(input.animations).some(a=>!['none','pulse','outline'].includes(a as string)))throw new Error('アニメーションが不正です');
 return structuredClone(input) as unknown as Theme;
}
export const themes=[loadTheme(standard),loadTheme(alternate)];
export const themeKey='ai-guild.dashboard.theme.v1';
export function chooseTheme(id:string|null){return themes.find(t=>t.id===id)??themes[0];}
export function themeStyle(t:Theme){return Object.fromEntries(Object.entries(t.colors).map(([k,v])=>[`--${k}`,v]));}
