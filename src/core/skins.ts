import {t,message as coreMessage,type UiMessage} from '../i18n';
import {ja,en} from '../i18n/catalog';
import {useEffect,useRef,useState} from 'react';
import {themeKey} from '../themes/theme';
export interface SkinSelection {readonly skinId:string;readonly version:string;readonly packageSha256:string}
export interface Bitmap {readonly assetId:string;readonly path:string;readonly width:number;readonly height:number;readonly frameWidth:number;readonly frameHeight:number;readonly columns:number;readonly rows:number}
export interface SkinCharacter {readonly characterId:string;readonly assetId:string;readonly prop:'paper'|'books'|'check'|'lamp'|'cabinet'}
export interface SkinLayout {readonly schemaVersion:1;readonly columns:number;readonly backgroundAssetId:string|null;readonly palette:Readonly<Record<'room'|'floor'|'wood'|'woodEdge'|'mint',string>>;readonly characters:readonly SkinCharacter[];readonly assignments:readonly {readonly roleId:string;readonly characterId:string}[]}
export interface SkinAnimation {readonly roleId:string;readonly status:string;readonly assetId:string;readonly row:number;readonly frame:number;readonly frameCount:number;readonly durationMs:number;readonly iterations:number}
export interface RenderSkin {readonly selection:SkinSelection;readonly layout:SkinLayout;readonly animations:{readonly schemaVersion:1;readonly animations:readonly SkinAnimation[]};readonly locales:Readonly<Record<string,{readonly schemaVersion:1;readonly namespace:'skin';readonly locale:string;readonly strings:Readonly<Record<string,string>>}>>;readonly assets:Readonly<Record<string,{readonly spec:Bitmap;readonly dataUrl:string}>>}
export interface SkinEntry {readonly selection:SkinSelection;readonly displayName:Readonly<Record<string,string>>;readonly description:Readonly<Record<string,string>>;readonly state:'AVAILABLE'|'CORRUPT'|'INCOMPATIBLE';readonly builtin:boolean;readonly reason:string|null}
export interface SkinLibrary {readonly contractVersion:1;readonly entries:readonly SkinEntry[];readonly selected:SkinSelection;readonly render:RenderSkin|null;readonly fallbackReason:string|null;readonly message:string|null;readonly installed?:SkinSelection}
export type SkinRequest={operation:'get';legacyTheme:string|null}|{operation:'import';bytes:number[]}|{operation:'select';selection:SkinSelection}|{operation:'fallback';selection:SkinSelection;reason:string};
export interface SkinClient {perform(request:SkinRequest):Promise<SkinLibrary>}
export const builtinSelection=(mint=false):SkinSelection=>({skinId:mint?'builtin-mint':'builtin-paper',version:'1.0.0',packageSha256:mint?'builtin:free-mint-test:v1':'builtin:free-standard:v1'});
export const selectionKey=(s:SkinSelection)=>`${s.skinId}@${s.version}#${s.packageSha256}`;
export const sameSelection=(a:SkinSelection,b:SkinSelection)=>selectionKey(a)===selectionKey(b);
export function skinText(s:RenderSkin|null,key:string,locale:'ja'|'en',fallback:string){return s?.locales[locale]?.strings[`skin.${key}`]||fallback;}
function initial():SkinLibrary{let mint=false;try{mint=localStorage.getItem(themeKey)==='free-mint-test';}catch{/* Built-ins remain available. */}return {contractVersion:1,selected:builtinSelection(mint),render:null,fallbackReason:null,message:null,entries:[false,true].map(m=>({selection:builtinSelection(m),displayName:{ja:m?'Mint':'Paper',en:m?'Mint':'Paper'},description:{ja:ja['skin.builtinDescription'],en:en['skin.builtinDescription']},state:'AVAILABLE',builtin:true,reason:null}))};}
export function skinProblemCode(input:string){
 const code=/^(?:Error: )?(SKIN_[A-Z_]+)\b/.exec(input)?.[1]??'';
 if(code==='SKIN_VERSION_CONFLICT')return 'skin.conflict' as const;
 if(['SKIN_INCOMPATIBLE','SKIN_VERSION','SKIN_SCHEMA_VERSION','SKIN_CORE_VERSION'].includes(code))return 'skin.incompatible' as const;
 if(['SKIN_ARCHIVE_SIZE','SKIN_TOTAL_SIZE','SKIN_FILE_SIZE','SKIN_PIXEL_LIMIT','SKIN_LIMIT','SKIN_ENTRY_LIMIT','SKIN_EXPANSION_LIMIT','SKIN_ASSET_LIMIT','SKIN_LIBRARY_LIMIT','SKIN_JSON_SIZE','SKIN_ZIP_SIZE'].includes(code))return 'skin.limit' as const;
 return 'skin.invalid' as const;
}
export function skinProblem(code:string){return t(skinProblemCode(code));}
export function useSkinLibrary(client?:SkinClient){
 const [library,setLibrary]=useState<SkinLibrary>(initial),[busy,setBusy]=useState(false),[message,setMessage]=useState<UiMessage>(''),[error,setError]=useState<UiMessage>('');const pending=useRef(false);
 function apply(v:SkinLibrary){if(v.contractVersion!==1||!v.entries.some(e=>e.builtin&&e.selection.skinId==='builtin-paper'))throw Error('SKIN_RESPONSE');setLibrary(v);if(v.fallbackReason)setError(coreMessage('skin.loadFallback'));}
 useEffect(()=>{let alive=true;if(client){let legacy:string|null=null;try{legacy=localStorage.getItem(themeKey);}catch{/* no legacy preference */}void client.perform({operation:'get',legacyTheme:legacy}).then(v=>{if(alive)apply(v);}).catch(()=>{if(alive){setLibrary(initial());setError(coreMessage('skin.loadFailed'));}});}return()=>{alive=false;};},[client]);
 async function act(request:SkinRequest){if(pending.current)return;pending.current=true;setBusy(true);setError('');setMessage('');try{if(!client)throw Error('SKIN_DESKTOP_REQUIRED');const v=await client.perform(request);apply(v);if(request.operation==='import')setMessage(coreMessage('skin.added'));if(request.operation==='select')setMessage(coreMessage('skin.selected'));return v;}catch(e){setError(coreMessage(skinProblemCode(String(e))));}finally{pending.current=false;setBusy(false);}}
 async function choose(selection:SkinSelection){if(client){if(!await act({operation:'select',selection}))return;}else if(selection.skinId==='builtin-paper'||selection.skinId==='builtin-mint'){setLibrary(v=>({...v,selected:selection,render:null}));}else return;
  if(selection.skinId==='builtin-paper'||selection.skinId==='builtin-mint'){try{localStorage.setItem(themeKey,selection.skinId==='builtin-mint'?'free-mint-test':'free-standard');}catch{setMessage(coreMessage('skin.unsaved'));}}
 }
 async function add(file:File){if(pending.current)return;if(file.size>8*1024*1024){setError(coreMessage(skinProblemCode('SKIN_ARCHIVE_SIZE')));return;}try{const bytes=new Uint8Array(await file.arrayBuffer());await act({operation:'import',bytes:Array.from(bytes)});}catch{setError(coreMessage('skin.readFailed'));}}
 function fallback(reason:string){const original=library.selected;setLibrary(v=>({...v,selected:builtinSelection(),render:null}));setError(coreMessage('skin.rendererFallback'));if(client)void client.perform({operation:'fallback',selection:original,reason}).then(apply).catch(()=>setError(coreMessage('skin.fallbackSaveFailed')));}
 return {library,busy,message,error,choose,add,fallback,canImport:!!client};
}
