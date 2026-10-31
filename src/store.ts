import {useSyncExternalStore} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {initialWorkflow,type Snapshot} from './model';
const native=Object.hasOwn(window,'__TAURI_INTERNALS__');
const preview:Snapshot={workflow:initialWorkflow,project:{name:'はじまりのギルド',path:'デスクトップ版で保存先を設定できます'},projects:[],mode:native?'real':'demo',connection:{status:'disconnected',version:'',account:'',error:null},job:null,history:[],events:[],approvals:[],instructions:[],positions:{},settings:{setupComplete:false,codexPath:'',workRoot:'',outputRoot:'',model:'',approvalPolicy:'on-request',inputPaths:[]},dataPath:native?'保存データを読み込み中…':'ブラウザ表示：ネイティブ機能は未接続'};
let snapshot:Snapshot=preview;const subscribers=new Set<()=>void>();
const emit=(s:Snapshot)=>{snapshot=s;subscribers.forEach(f=>f());};
export const isNative=native;
let initialized=false;
export async function initializeStore(){if(initialized)return;initialized=true;if(native){let unsubscribe:(()=>void)|undefined;try{unsubscribe=await listen<Snapshot>('guild:snapshot',e=>emit(e.payload));emit(await invoke<Snapshot>('get_snapshot'));}catch(e){unsubscribe?.();initialized=false;throw e;}}}
export function useGuild(){return useSyncExternalStore(f=>{subscribers.add(f);return()=>subscribers.delete(f);},()=>snapshot);}
export async function command<T=void>(name:string,args:Record<string,unknown>={}):Promise<T>{
 if(native)return invoke<T>(name,args);
 if(name==='set_position'){emit({...snapshot,positions:{...snapshot.positions,[args.nodeId as string]:{x:Number(args.x),y:Number(args.y)}}});return undefined as T;}
 if(name==='reset_layout'){emit({...snapshot,positions:{}});return undefined as T;}
 if(name==='save_workflow'){emit({...snapshot,workflow:args.workflow as Snapshot['workflow'],positions:{}});return undefined as T;}
 throw new Error('ブラウザは画面検証用です。実行・SQLite保存・Codex接続はAI GUILDのデスクトップ版で利用できます。');
}

