import type {CoreClient} from '../core/contract';
import {openPresentation,type PresentationEvent,type PresentationSnapshot} from '../core/presentation';

export interface StageFrame {readonly snapshot:PresentationSnapshot|null;readonly handoffs:readonly PresentationEvent[];readonly visible:boolean}
type View=Awaited<ReturnType<typeof openPresentation>>;
/** Trusted subscription owner. Timers only remove visual pulses; they never advance work. */
export class StagePresentationFeed {
 private view:View|null=null;
 private epoch=0;
 private disconnected=false;
 private timer:ReturnType<typeof setTimeout>|null=null;
 private frame:StageFrame={snapshot:null,handoffs:[],visible:true};
 constructor(private readonly publish:(frame:StageFrame)=>void){}
 private update(frame:StageFrame){this.frame=frame;this.publish(frame);}
 clearHandoffs(){if(this.timer)clearTimeout(this.timer);this.timer=null;this.update({...this.frame,handoffs:[]});}
 async select(client:CoreClient,jobId:string|null){
  const epoch=++this.epoch;this.disconnected=false;this.view?.close();this.view=null;this.clearHandoffs();this.update({...this.frame,snapshot:null});
  if(!jobId)return;
  const view=await openPresentation(client,jobId,(snapshot,handoffs)=>{
   if(epoch!==this.epoch)return;
   if(snapshot&&this.disconnected)snapshot={...snapshot,liveEvidence:'RESYNC_REQUIRED',liveReasonCode:'NOTIFICATION_DISCONNECTED'};
   const fresh=this.frame.visible&&snapshot?.liveEvidence==='CURRENT'?handoffs:[];
   const retained=this.frame.visible&&snapshot?.liveEvidence==='CURRENT'?this.frame.handoffs:[];
   const merged=[...retained,...fresh.filter(e=>!retained.some(v=>v.sourceEventId===e.sourceEventId))];
   this.update({...this.frame,snapshot,handoffs:merged});
   if(fresh.length){if(this.timer)clearTimeout(this.timer);this.timer=setTimeout(()=>this.clearHandoffs(),1250);}
  });
  if(epoch!==this.epoch){view.close();return;}this.view=view;if(this.disconnected)view.disconnect();
 }
 setVisible(visible:boolean){this.clearHandoffs();this.update({...this.frame,visible});if(visible)void this.view?.resync();}
 disconnect(){this.disconnected=true;this.clearHandoffs();this.view?.disconnect();}
 async reconnect(){this.disconnected=false;this.clearHandoffs();await this.view?.reconnect();}
 close(){++this.epoch;this.view?.close();this.view=null;if(this.timer)clearTimeout(this.timer);this.timer=null;}
}
