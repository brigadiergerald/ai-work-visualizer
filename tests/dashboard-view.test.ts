import {expect,it} from 'vitest';
import {automaticPreview,deliveredProduct,verifiedPreview,visiblePreview} from '../src/dashboard/dashboard-view';
import type {Job} from '../src/core/contract';
const file={name:'post.txt',path:'C:/job-a/post.txt',sha256:'a'.repeat(64),bytes:20,role:'product',integrity:'verified' as const};
const job=():Job=>({id:'job-a',state:'completed',quality:{filesVerified:true},artifacts:[file]} as Job);
it('requires formal Job completion and verified product files, never Task completion alone',()=>{
 const j=job();expect(automaticPreview(j)).toEqual(file);
 for(const state of ['queued','running','awaiting_approval','failed','unknown'] as const)expect(deliveredProduct({...j,state})).toBeUndefined();
 expect(automaticPreview({...j,quality:{...j.quality,filesVerified:false}})).toBeUndefined();
 expect(automaticPreview({...j,artifacts:[{...file,integrity:'failed'}]})).toBeUndefined();
 expect(automaticPreview({...j,artifacts:[{...file,role:'evidence'}]})).toBeUndefined();
 expect(automaticPreview({...j,artifacts:[{...file,name:'product.pdf'}]})).toBeUndefined();
});
it('rejects unregistered paths and hash drift before displaying a read-only Core response',()=>{
 const result={text:'Exact saved content',sha256:file.sha256};
 expect(verifiedPreview(job(),file,result).text).toBe(result.text);
 expect(()=>verifiedPreview(job(),file,{...result,sha256:'b'.repeat(64)})).toThrow();
 expect(()=>verifiedPreview({...job(),artifacts:[]},file,result)).toThrow();
 expect(()=>verifiedPreview(job(),{...file,path:'C:/another/post.txt'},result)).toThrow();
});
it('does not expose a previous Job preview, even for a path and hash reused by another Job',()=>{
 const j=job(),preview=verifiedPreview(j,file,{text:'Saved',sha256:file.sha256});
 expect(visiblePreview(j,preview)).toBe(true);
 expect(visiblePreview({...j,id:'job-b'},preview)).toBe(false);
 expect(visiblePreview({...j,artifacts:[{...file,sha256:'b'.repeat(64)}]},preview)).toBe(false);
 expect(visiblePreview(null,preview)).toBe(false);
});
