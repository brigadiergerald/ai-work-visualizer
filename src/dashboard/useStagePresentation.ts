import {useEffect,useRef,useState} from 'react';
import type {CoreClient} from '../core/contract';
import {StagePresentationFeed,type StageFrame} from './stage-presentation';

export function useStagePresentation(client:CoreClient,jobId:string|null,purpose:string,theme:string){
 const [frame,setFrame]=useState<StageFrame>({snapshot:null,handoffs:[],visible:!document.hidden});
 const feed=useRef<StagePresentationFeed|null>(null);
 useEffect(()=>{
  const owner=new StagePresentationFeed(setFrame);feed.current=owner;
  owner.setVisible(!document.hidden);void owner.select(client,jobId);
  const visibility=()=>owner.setVisible(!document.hidden),offline=()=>owner.disconnect(),online=()=>void owner.reconnect();
  document.addEventListener('visibilitychange',visibility);window.addEventListener('offline',offline);window.addEventListener('online',online);
  return()=>{owner.close();document.removeEventListener('visibilitychange',visibility);window.removeEventListener('offline',offline);window.removeEventListener('online',online);if(feed.current===owner)feed.current=null;};
 },[client,jobId]);
 useEffect(()=>{feed.current?.clearHandoffs();},[purpose,theme]);
 return {...frame,reconnect:()=>feed.current?.reconnect()};
}
