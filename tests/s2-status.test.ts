import {it,expect} from 'vitest';
import {practicalCompletion,coreStageLabels,type Job} from '../src/model';
it('does not present internal completed as practical completion without all local gates',()=>{
 const j={status:'completed',collaboration:{stage:'D_MACHINE_PASS_DISPLAY_PENDING'},quality:{machineStatus:'PASS',displayStatus:'NOT_VERIFIED'}} as Job;
 expect(practicalCompletion(j)).toBe(coreStageLabels.D_MACHINE_PASS_DISPLAY_PENDING);
 j.collaboration!.stage='DONE_LOCAL_ONLY';expect(practicalCompletion(j)).toBe('要介入');
 j.quality!.displayStatus='PASS_LOCAL_ONLY';expect(practicalCompletion(j)).toContain('実AI連携は未検証');
 expect(practicalCompletion(j,false)).toContain('実file照合に失敗');
});

it('keeps work completion distinct from a partial QA pass',()=>{
 const j={status:'running',practical:{version:3},quality:{machineStatus:'PASS',displayStatus:'NOT_VERIFIED'}} as Job;
 expect(practicalCompletion(j)).toContain('実務完了は未確定');
 j.status='completed';expect(practicalCompletion(j)).toContain('内容と表示');
});
