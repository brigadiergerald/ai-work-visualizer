import {it,expect} from 'vitest';
import {handoffsForUpdate,type HandoffCursor,type GuildEvent} from '../src/model';
const event=(eventId:string,jobId:string,type='handoff_received',edgeId:string|null='research-writing'):GuildEvent=>({eventId,jobId,eventType:type,edgeId,timestamp:'2026-10-27T01:50:15Z',workflowId:'qa',nodeId:'writing',status:'handing_off',message:'actual host event',artifactPaths:[],mode:'real'});
it('does not animate saved handoffs on hydration, history selection or restart',()=>{
 const cursor:HandoffCursor={jobId:undefined,primed:false,seen:new Set()};const old=event('persisted','old');
 expect(handoffsForUpdate([],undefined,cursor).events).toEqual([]);
 expect(handoffsForUpdate([old],'old',cursor)).toEqual({reset:true,events:[]});
 expect(handoffsForUpdate([old],'old',cursor).events).toEqual([]);
 expect(handoffsForUpdate([old,event('persisted-other','other')],'other',cursor).events).toEqual([]);
});
it('animates each newly observed matching handoff once and rejects other jobs/non-handoff events',()=>{
 const cursor:HandoffCursor={jobId:'current',primed:false,seen:new Set()};handoffsForUpdate([],'current',cursor);
 const real=event('new','current'),other=event('other','old'),message=event('message','current','node_started'),withoutEdge=event('missing-edge','current','handoff_started',null);
 const pending=event('pending','current','handoff_started'),failed=event('failed','current','handoff_failed');
 const events=[real,other,message,withoutEdge,pending,failed];expect(handoffsForUpdate(events,'current',cursor).events.map(e=>e.eventId)).toEqual(['new']);expect(handoffsForUpdate(events,'current',cursor).events).toEqual([]);
});
