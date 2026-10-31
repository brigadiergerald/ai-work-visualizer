/** Read-only Core projection. Narrative text is literal saved production output. */
export type DecisionKind='theme'|'direction'|'writing'|'inspection';
export interface DecisionRecord {
 kind:DecisionKind;jobId:string;nodeId:string;taskId:string|null;attempt:number|null;roleId:string|null;stageState:string;
 stageError?:string|null;sourceFields?:{requirements:string[];productionConstraints:string[]}|null;
 status:'NO_RECORD'|'RECORDED'|'INVALID_RECORD'|'SOURCE_FAILED';recordedAt:string|null;
 source:{name:string;path:string;sha256:string;bytes:number;integrity:'verified'|'failed'}|null;
 note:{summary:string;reason:string;details:string[]}|null;
 inspection:{version:1;status:string;basis:string;checks:{criterion:string;result:string;observed:number|null}[];unverified:string[];nextStageBasis:string;humanContentApproval:string;publication:string}|null;
}
export interface DecisionView {version:1;jobId:string;records:DecisionRecord[];humanContentApproval:'PENDING'|'RECORDED_SEPARATELY'|'NOT_PERFORMED';publication:'NOT_PERFORMED'}
