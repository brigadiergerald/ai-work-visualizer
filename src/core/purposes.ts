import {ja} from '../i18n/catalog';
/** Additive Purpose contract. Workflow selection and validation belong to Core. */
import type {CompletionPlanInput} from './integrations';
import type {CoreSnapshot,Job} from './contract';
export type PurposeId='social-text'|'research';
export type ContentLanguage='ja'|'en';
export interface SocialTextInput {theme:string;contentLanguage:ContentLanguage;context?:string}
export interface PurposeDefinition {id:PurposeId;definitionVersion:string;roleMapVersion:string|null;label:string;description:string;availability:'READY'|'PLANNED';reasonCode:string|null;publicationMode:'PREPARATION_ONLY';inputSchema:{type:'object';additionalProperties:false;required:string[];maxInstructionUtf8Bytes?:number;properties:Record<string,unknown>}}
export interface PurposeCatalog {contractVersion:1;purposes:PurposeDefinition[]}
export interface PurposeJobView {jobId:string;purposeId:PurposeId|null;definitionVersion:string|null;roleMapVersion:string|null;contentLanguage:ContentLanguage|null;inputSha256:string|null;contextSource:'PERSISTED_NEW_JOB'|'LEGACY_SNS_PROJECTION'|'UNCLASSIFIED'}
export type PurposeAction={action:'listPurposes'}|{action:'startPurpose';purposeId:PurposeId;definitionVersion:string;input:SocialTextInput;completionPlan?:CompletionPlanInput};
export const initialSocialInput=():SocialTextInput=>({theme:'',context:'',contentLanguage:'ja'});
export const utf8Bytes=(text:string)=>new TextEncoder().encode(text).length;
export function socialInputIssueCode(input:SocialTextInput):'social.themeRequired'|'social.languageRequired'|'social.inputLimit'|null{
 if(!input.theme.trim())return 'social.themeRequired';
 if(!['ja','en'].includes(input.contentLanguage))return 'social.languageRequired';
 if(utf8Bytes(input.theme)>64000||utf8Bytes(input.context??'')>64000)return 'social.inputLimit';
 return null;
}
export function socialInputIssue(input:SocialTextInput):string{const code=socialInputIssueCode(input);return code?ja[code]:'';}
export const currentJobLocked=(s:CoreSnapshot)=>s.jobs.some(j=>j.id===s.currentJobId&&['queued','running','awaiting_approval','paused','unknown'].includes(j.state));
export const purposeOf=(j:Job):PurposeId|null=>j.purpose?.purposeId??(j.workflowId==='sns-draft-v1'?'social-text':null);
export const historyFor=(jobs:Job[],filter:'all'|PurposeId)=>filter==='all'?jobs:jobs.filter(j=>purposeOf(j)===filter);
export interface PendingPurposeStart {profile:string;beforeIds:string[];inputSha256:string;definitionVersion:string;contentLanguage:ContentLanguage}
export const pendingStartKey='ai-guild.purpose-start-pending.v1';
export async function socialInputHash(input:SocialTextInput):Promise<string>{
 // serde_json::Value serializes object keys in lexical order. Values remain verbatim.
 const canonical=JSON.stringify({contentLanguage:input.contentLanguage,...(input.context!==undefined?{context:input.context}:{}),theme:input.theme});
 return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(canonical))),b=>b.toString(16).padStart(2,'0')).join('');
}
export function reconcilePurposeStart(p:PendingPurposeStart,s:CoreSnapshot):Job|null{
 if(p.profile!==s.dataPath)return null;
 const matches=s.jobs.filter(j=>!p.beforeIds.includes(j.id)&&j.purpose?.contextSource==='PERSISTED_NEW_JOB'&&j.purpose.purposeId==='social-text'&&j.purpose.inputSha256===p.inputSha256&&j.purpose.definitionVersion===p.definitionVersion&&j.purpose.contentLanguage===p.contentLanguage);
 return matches.length===1?matches[0]:null;
}
export const confirmedNotStarted=(error:unknown)=>typeof error==='string'&&error.startsWith('[PURPOSE_NOT_STARTED] ');
