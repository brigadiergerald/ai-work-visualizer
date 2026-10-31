import type {ProviderConnection} from './providers';
export type PostingState='REVIEW_REQUIRED'|'SUBMISSION_STARTED'|'PROVIDER_ACCEPTED'|'SCHEDULED'|'PUBLISHING'|'PUBLISHED_REPORTED'|'FAILED'|'UNKNOWN';
export type DeliveryMode='scheduled'|'immediate';
export interface ManualPost {deliveryMode?:DeliveryMode;id:string;revision:number;jobId:string;connectionId:string;connectionRevision:number;accountId:string;workspaceId:string;channelId:string;channelName:string;text:string;textSha256:string;sourceSha256:string;dueAt:string;timeZone:'Asia/Tokyo';createdAt:string;expiresAt:string;reviewToken:string;state:PostingState;approvalId:string|null;submissionId:string|null;receiptId:string|null;providerStatus:string|null;externalLink:string|null;error:string|null;history:{at:string;event:string;state:string;error?:string|null;receiptId?:string|null}[]}
export interface PostingView {version:1;jobId:string;posts:ManualPost[];scope:string}
export type PostingAction={operation:'get';jobId:string}|{operation:'prepare';jobId:string;connectionId:string;connectionRevision:number;channelId:string;dueAt:string}|{operation:'prepareNow';jobId:string;connectionId:string;connectionRevision:number;channelId:string}|{operation:'submit';recordId:string;revision:number;reviewToken:string}|{operation:'check';recordId:string;revision:number};
export const postMode=(p:ManualPost):DeliveryMode=>p.deliveryMode??'scheduled';
export function tokyoToUtc(value:string):string|null{
 if(!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(value))return null;
 const dt=new Date(`${value}:00+09:00`);if(!Number.isFinite(dt.getTime()))return null;
 if(new Date(dt.getTime()+9*3600_000).toISOString().slice(0,16)!==value)return null;
 return dt.toISOString();
}
export const threadsTargets=(connections:ProviderConnection[])=>connections.filter(c=>c.providerId==='buffer'&&c.state==='TARGETS_VERIFIED').flatMap(c=>c.channels.filter(ch=>ch.platform==='threads'&&ch.workspaceId===c.selectedWorkspaceId&&c.selectedChannelIds.includes(ch.id)).map(channel=>({connection:c,channel})));
export const maySubmit=(p:ManualPost,jobId:string,confirmed:boolean)=>p.jobId===jobId&&p.state==='REVIEW_REQUIRED'&&confirmed;

export const postingDate=(value:string,locale:string)=>new Date(value).toLocaleString(locale==='ja'?'ja-JP':'en-US',{timeZone:'Asia/Tokyo',year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',hour12:false});
