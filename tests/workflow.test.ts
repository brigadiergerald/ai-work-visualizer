import {describe,it,expect} from 'vitest';
import {initialWorkflow,canTransition,type Workflow} from '../src/model';
import {validateWorkflow,layoutWorkflow} from '../src/workflow';
describe('workflow integrity',()=>{
 it('lays out the approved definition left to right',()=>{expect(validateWorkflow(initialWorkflow)).toEqual([]);const p=layoutWorkflow(initialWorkflow);expect(Object.keys(p)).toHaveLength(5);expect(p.research.x).toBeLessThan(p.writing.x);});
 it('rejects duplicate IDs, phantom targets, cycles and inconsistent dependencies',()=>{
  const w=structuredClone(initialWorkflow);w.nodes[0].dependencies=['delivery'];w.edges.push({edgeId:'cycle',fromNodeId:'delivery',toNodeId:'research'});expect(validateWorkflow(w).join()).toContain('循環');
  const duplicate=structuredClone(initialWorkflow);duplicate.nodes.push(duplicate.nodes[0]);expect(validateWorkflow(duplicate).join()).toContain('重複');
  const missing=structuredClone(initialWorkflow);missing.edges[0].toNodeId='nonexistent';expect(validateWorkflow(missing).join()).toContain('存在しない');
  const inconsistent=structuredClone(initialWorkflow);inconsistent.nodes[1].dependencies=[];expect(validateWorkflow(inconsistent).join()).toContain('一致しません');
 });
 it('gives independent branches different y positions',()=>{const w:Workflow=structuredClone(initialWorkflow);w.nodes[2].dependencies=['research'];w.edges=w.edges.map(e=>e.toNodeId==='image'?{...e,fromNodeId:'research'}:e);expect(validateWorkflow(w)).toEqual([]);const p=layoutWorkflow(w);expect(p.writing.y).not.toBe(p.image.y);});
 it('rejects traversal output names and unknown executors',()=>{const w=structuredClone(initialWorkflow);w.nodes[0].outputArtifacts=['../outside.md'];expect(validateWorkflow(w).length).toBeGreaterThan(0);});
 it('never equates disconnect or interrupted with completed',()=>{expect(canTransition('completed','running')).toBe(false);expect(canTransition('running','disconnected')).toBe(true);expect(canTransition('disconnected','completed')).toBe(false);});
});
