import defaultWorkflow from './default-workflow.json';
export type Status='queued'|'running'|'handing_off'|'awaiting_approval'|'paused'|'completed'|'failed'|'cancelled'|'disconnected';
export type Executor='codex'|'manual'|'save'|'quality';
export interface WorkflowNode {nodeId:string;role:string;label:string;spriteKey:string;workInstruction:string;dependencies:string[];inputArtifacts:string[];outputArtifacts:string[];executor:Executor}
export interface WorkflowEdge {edgeId:string;fromNodeId:string;toNodeId:string}
export interface Workflow {workflowId:string;workflowName:string;nodes:WorkflowNode[];edges:WorkflowEdge[]}
export interface Artifact {path:string;name:string;nodeId:string;sha256:string;bytes:number;kind:string}
export interface Run {status:Status;startedAt:string|null;endedAt:string|null;output:string;error:string|null;attempt:number}
export interface Job {jobId:string;workflowId:string;workflow:Workflow;status:Status;mode:'real'|'demo'|'local_only';instruction:string;startedAt:string;endedAt:string|null;workspace:string;threadId:string|null;turnId:string|null;activeNodeId:string|null;nodes:Record<string,Run>;artifacts:Artifact[];stopRequested:boolean;practical?:{version:number;originalInstruction:string;turnStarts:number;maxTurnStarts:number};quality?:{artifactSha256:string;machineStatus:string;displayStatus:string;checkedAt:string;checks:string[]};recovery?:{stage:string;startedAt:string;captureSha256:string};collaboration?:{scope:string;stage:string;identity:{jobId:string;taskId:string;attempt:number};receipts:{artifactId:string;sha256:string;contentReview:{verdict:string}|null}[];failures:{stage:string;symptom:string;resolved:boolean}[]}}
export interface GuildEvent {eventId:string;timestamp:string;jobId:string;workflowId:string;nodeId:string|null;eventType:string;status:Status;message:string;artifactPaths:string[];edgeId?:string|null;errorCode?:string|null;mode:'real'|'demo'|'local_only'|'system'}
export const eventsForJob=(events:GuildEvent[],jobId?:string)=>events.filter(e=>e.jobId===(jobId??''));
export interface HandoffCursor {jobId:string|undefined;primed:boolean;seen:Set<string>}
export function handoffsForUpdate(events:GuildEvent[],jobId:string|undefined,cursor:HandoffCursor){
 const reset=!cursor.primed||cursor.jobId!==jobId;
 const fresh=reset?[]:events.filter(e=>!cursor.seen.has(e.eventId)&&e.jobId===jobId&&e.eventType==='handoff_received'&&!!e.edgeId);
 cursor.jobId=jobId;cursor.primed=true;events.forEach(e=>cursor.seen.add(e.eventId));
 return {reset,events:fresh};
}
export interface Approval {id:string;requestId:unknown;jobId:string;nodeId:string;kind:string;title:string;details:string;params:Record<string,unknown>;resolved:boolean;decision:string|null}
export interface Instruction {id:string;jobId:string;kind:'initial'|'additional'|'approval';text:string;timestamp:string}
export interface Project {name:string;path:string}
export interface Connection {status:'disconnected'|'connecting'|'connected';version:string;account:string;error:string|null}
export interface ProductSettings {setupComplete:boolean;codexPath:string;workRoot:string;outputRoot:string;model:string;approvalPolicy:string;inputPaths:string[]}
export interface Snapshot {workflow:Workflow;projects:Project[];project:Project;mode:'real'|'demo';connection:Connection;job:Job|null;history:Job[];events:GuildEvent[];approvals:Approval[];instructions:Instruction[];positions:Record<string,{x:number;y:number}>;dataPath:string;settings:ProductSettings}
export const statusLabels:Record<Status,string>={queued:'待機',running:'実行中',handing_off:'受け渡し中',awaiting_approval:'要確認',paused:'一時停止',completed:'完了',failed:'失敗',cancelled:'停止',disconnected:'状態不明'};
export const roles:Record<string,{name:string;color:string;title:string}>={scholar:{name:'リオ',color:'#81c9ba',title:'調査・要件整理'},scribe:{name:'フィン',color:'#eea096',title:'計画・設計'},artist:{name:'ミモ',color:'#b7a1de',title:'制作・実装'},analyst:{name:'ノア',color:'#93b8df',title:'品質検査・修正'},messenger:{name:'ルカ',color:'#e6ca85',title:'保存・納品'}};
export const initialWorkflow:Workflow=defaultWorkflow as Workflow;
const transitions:Record<Status,Status[]>={queued:['running','cancelled','disconnected'],running:['handing_off','awaiting_approval','paused','completed','failed','cancelled','disconnected'],handing_off:['running','completed','failed','cancelled','disconnected'],awaiting_approval:['running','paused','cancelled','failed','disconnected'],paused:['queued','running','cancelled','failed','disconnected'],completed:[],failed:['queued','cancelled'],cancelled:['queued'],disconnected:['running','paused','failed','cancelled']};
export const canTransition=(from:Status,to:Status)=>from===to||transitions[from].includes(to);


export const coreStageLabels:Record<string,string>={PREPARED:'工程準備',RECEIVING:'素材受領中',MATERIAL_RECEIVED:'素材受領・品質検査待ち',MATERIAL_C_PASS:'素材C合格',INTEGRATING:'完成品統合中',D_MACHINE_PASS_DISPLAY_PENDING:'完成品の表示検査待ち',DONE_LOCAL_ONLY:'完了（LOCAL_ONLY）'};
export function practicalCompletion(job:Job,available?:boolean):string {
 if(available===false)return '要介入：完成品の実file照合に失敗';
 if(job.collaboration){if(job.status==='completed'&&job.collaboration.stage==='DONE_LOCAL_ONLY'&&job.quality?.machineStatus==='PASS'&&job.quality?.displayStatus==='PASS_LOCAL_ONLY')return 'LOCAL_ONLY完成品・実AI連携は未検証';return job.collaboration.stage==='DONE_LOCAL_ONLY'?'要介入':coreStageLabels[job.collaboration.stage]??'要介入';}
 if(job.practical&&job.status!=='completed')return '工程進行中・実務完了は未確定';
 return job.quality?.machineStatus==='PASS'?'完成品の機械検査PASS・内容と表示の実務確認は別判定':'完成品は未検証';
}
