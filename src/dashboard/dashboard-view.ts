import type {Artifact,Job} from '../core/contract';

/** Display-only decisions; never change a Job, approval or execution state. */
export const newRequestSelection='new-request';
export function deliveredProduct(job:Job|null):Artifact|undefined{
 return job?.state==='completed'&&job.quality.filesVerified
  ?job.artifacts.find(a=>a.role==='product'&&a.integrity==='verified'):undefined;
}
export function automaticPreview(job:Job|null):Artifact|undefined{
 return deliveredProduct(job)?job!.artifacts.find(a=>a.role==='product'&&a.integrity==='verified'&&/\.(md|txt)$/i.test(a.name)):undefined;
}
export interface TextPreview {jobId:string;path:string;hash:string;text:string}
export function verifiedPreview(job:Job|null,expected:Artifact,result:{text:string;sha256:string}):TextPreview{
 if(!job||typeof result?.text!=='string'||result.sha256!==expected.sha256||!job.artifacts.some(a=>a.role==='product'&&a.integrity==='verified'&&a.path===expected.path&&a.sha256===expected.sha256))throw Error('PREVIEW_ARTIFACT_CHANGED');
 return {jobId:job.id,path:expected.path,hash:result.sha256,text:result.text};
}
export function visiblePreview(job:Job|null,preview:TextPreview|null):boolean{
 return !!job&&!!preview&&preview.jobId===job.id&&job.artifacts.some(a=>a.role==='product'&&a.integrity==='verified'&&a.path===preview.path&&a.sha256===preview.hash);
}
