import {useSyncExternalStore} from 'react';
import {ja,en} from './catalog';
export type UiLocale='ja'|'en';
export type MessageKey=keyof typeof ja;
type Slots<S extends string>=S extends `${string}{${infer P}}${infer Rest}`?P|Slots<Rest>:never;
type Params<K extends MessageKey>=Record<Slots<(typeof ja)[K]>,string|number>;
type Args<K extends MessageKey>=[Slots<(typeof ja)[K]>] extends [never]?[]:[params:Params<K>];
export const presentationKey='ai-guild.dashboard.presentation.v1';
export const normalizeLocale=(v:unknown):UiLocale=>v==='en'?'en':'ja';
export function readLocale(storage:Pick<Storage,'getItem'>|undefined):UiLocale{
 try {return normalizeLocale(JSON.parse(storage?.getItem(presentationKey)??'null')?.uiLocale);}catch{return 'ja';}
}
export function persistLocale(storage:Pick<Storage,'getItem'|'setItem'>,value:UiLocale):boolean{
 try {let settings:Record<string,unknown>={};try{const v:unknown=JSON.parse(storage.getItem(presentationKey)??'null');if(v&&typeof v==='object'&&!Array.isArray(v))settings=v as Record<string,unknown>;}catch{/* Explicit user selection can replace malformed presentation preferences. */}
  storage.setItem(presentationKey,JSON.stringify({...settings,uiLocale:value}));return true;
 }catch{return false;}
}
const storage=()=>{try{return typeof localStorage==='undefined'?undefined:localStorage;}catch{return undefined;}};
let locale=readLocale(storage());const listeners=new Set<()=>void>();
export const getUiLocale=()=>locale;
function applyDocumentLocale(){if(typeof document!=='undefined'){document.documentElement.lang=locale;document.title=catalogs[locale]['shell.documentTitle'];}}
export function setUiLocale(value:UiLocale){locale=normalizeLocale(value);const s=storage(),saved=!!s&&persistLocale(s,locale);applyDocumentLocale();listeners.forEach(fn=>fn());return saved;}
export function useUiLocale(){return useSyncExternalStore(fn=>{listeners.add(fn);return()=>{listeners.delete(fn);};},getUiLocale,()=> 'ja' as const);}
export const catalogs={ja,en} as const;
applyDocumentLocale();
/** Fallback is trusted Core Japanese, then an explicit UNKNOWN. Skin data is never consulted. */
export function formatCore(active:UiLocale,key:string,params:Readonly<Record<string,string|number>>={},override?:Readonly<Record<string,string>>):string{
 const base=Object.hasOwn(ja,key)?(ja as Readonly<Record<string,string>>)[key]:undefined;
 if(!base)return active==='en'?en['state.unknown']:ja['state.unknown'];
 const candidate=override?.[key]??(catalogs[active] as Readonly<Record<string,string>>)[key];
 const slots=(s:string)=>[...s.matchAll(/\{(\w+)\}/g)].map(m=>m[1]).sort().join('|');
 const text=candidate?.trim()&&slots(candidate)===slots(base)?candidate:base;
 if([...base.matchAll(/\{(\w+)\}/g)].some(m=>!Object.hasOwn(params,m[1])))return active==='en'?en['state.unknown']:ja['state.unknown'];
 return text.replace(/\{(\w+)\}/g,(_,p:string)=>String(params[p]));
}
export function t<K extends MessageKey>(key:K,...args:Args<K>):string{return formatCore(locale,key,args[0]);}
export interface CoreMessage {readonly key:MessageKey;readonly params?:Readonly<Record<string,string|number>>}
export type UiMessage=string|CoreMessage;
export function message<K extends MessageKey>(key:K,...args:Args<K>):CoreMessage{return {key,params:args[0]};}
export function uiMessage(value:UiMessage){return typeof value==='string'?value:formatCore(locale,value.key,value.params);}
export class CoreUiError extends Error {readonly ui:CoreMessage;constructor(key:MessageKey){super(key);this.ui={key};}}
export function errorMessage(error:unknown):UiMessage{return error instanceof CoreUiError?error.ui:String(error);}
export const formatDate=(value:string)=>{const d=new Date(value);return Number.isNaN(d.getTime())?value:d.toLocaleString(locale==='ja'?'ja-JP':'en-US');};
export function coreLabel(group:string,value:string){const key=`${group}.${value}`;return Object.hasOwn(ja,key)?formatCore(locale,key):`${formatCore(locale,'state.unknown')} (${value})`;}
/** Only explicitly registered semantic IDs are localized; unknown descriptors stay literal. */
export function coreText(group:string,value:string,original:string){const key=`${group}.${value}`;return Object.hasOwn(ja,key)?formatCore(locale,key):original;}
/** Namespace-limited skin metadata. It can never resolve a Core key. */
export function localizedSkinText(values:Readonly<Record<string,string>>|undefined,fallback:string){return values?.[locale]||values?.ja||values?.en||fallback;}
