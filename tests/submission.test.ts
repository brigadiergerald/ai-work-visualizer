import {describe,it,expect} from 'vitest';
import {submissionBlocked} from '../src/submission';
describe('normal start safety for button and keyboard',()=>{
 const ready={pending:false,busy:false,historical:false,responsePending:false,initial:true,approval:false,locked:false,composing:false};
 it('permits one normal start and blocks duplicates, IME, history and active jobs',()=>{
  expect(submissionBlocked(ready)).toBe(false);
  for(const key of ['pending','busy','historical','responsePending','locked','composing'] as const)expect(submissionBlocked({...ready,[key]:true}),key).toBe(true);
 });
 it('lets questions and additional instructions use their proper operation while a job is locked',()=>{
  expect(submissionBlocked({...ready,locked:true,approval:true})).toBe(false);
  expect(submissionBlocked({...ready,locked:true,initial:false})).toBe(false);
  expect(submissionBlocked({...ready,locked:true,approval:true,responsePending:true})).toBe(true);
 });
});
