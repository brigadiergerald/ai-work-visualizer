import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import type {CoreClient,CoreSnapshot,Action} from './contract';
import type {PresentationSnapshot,PresentationNotice} from './presentation';
export const emptySnapshot:CoreSnapshot={contractVersion:1,connection:{agentId:'codex',state:'disconnected',version:'',account:'',error:null},currentJobId:null,jobs:[],settings:{setupComplete:false,codexPath:'',workRoot:'',outputRoot:'',model:'',approvalPolicy:'on-request',inputPaths:[]},mode:'real',dataPath:''};
export const client:CoreClient={
 skins:Object.hasOwn(window,'__TAURI_INTERNALS__')?{perform:request=>invoke('skin_action',{request})}:undefined,
 environment:Object.hasOwn(window,'__TAURI_INTERNALS__')?'desktop':'preview',
 async getSnapshot(){return this.environment==='desktop'?invoke<CoreSnapshot>('core_snapshot'):emptySnapshot;},
 async subscribe(fn){return this.environment==='desktop'?listen<CoreSnapshot>('core:snapshot',e=>fn(e.payload)):()=>{};},
 async getPresentation(jobId){if(this.environment!=='desktop')throw new Error('PRESENTATION_UNAVAILABLE');return invoke<PresentationSnapshot>('core_presentation_snapshot',{jobId});},
 async subscribePresentation(fn,onError){if(this.environment!=='desktop')throw new Error('PRESENTATION_UNAVAILABLE');return listen<PresentationNotice>('core:presentation',e=>{try{fn(e.payload);}catch(error){onError?.(error);}});},
 async execute<T>(request:Action){if(this.environment!=='desktop')throw new Error('ブラウザ表示では実行できません。デスクトップ版でCodexを接続してください。');return invoke<T>('core_action',{request});}
};
