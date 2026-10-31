import {it,expect} from 'vitest';
import {eventsForJob,type GuildEvent} from '../src/model';
const event=(eventId:string,jobId:string,eventType:string):GuildEvent=>({eventId,jobId,eventType,timestamp:'2026-10-26T12:00:00Z',workflowId:'test',nodeId:null,status:'queued',message:eventId,artifactPaths:[],mode:'system'});
it('keeps a prior quest connection event out of the selected current or historical quest',()=>{
 const events=[event('global','', 'connection'),event('old-connection','old','connection'),event('old-run','old','node_started'),event('new-run','new','node_started')];
 expect(eventsForJob(events,'new').map(e=>e.eventId)).toEqual(['new-run']);
 expect(eventsForJob(events,'old').map(e=>e.eventId)).toEqual(['old-connection','old-run']);
 expect(eventsForJob(events).map(e=>e.eventId)).toEqual(['global']);
 expect(events).toHaveLength(4);
});
