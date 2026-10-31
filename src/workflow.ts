import dagre from '@dagrejs/dagre';
import type { Workflow } from './model';
export function validateWorkflow(w:Workflow):string[]{
 const errors:string[]=[];
 if(!w||typeof w!=='object'||!w.workflowId||!w.workflowName||!Array.isArray(w.nodes)||!Array.isArray(w.edges))return ['ワークフローの形式が正しくありません。'];
 if(w.nodes.length<1||w.nodes.length>40)errors.push('工程数は1〜40にしてください。');
 const ids=new Set<string>(),edges=new Set<string>();
 for(const n of w.nodes){if(!n||!n.nodeId||ids.has(n.nodeId))errors.push('工程IDが空、または重複しています。');ids.add(n?.nodeId);if(!/^[a-zA-Z0-9_-]+$/.test(n?.nodeId||''))errors.push('工程IDは半角英数字・ハイフン・アンダースコアにしてください。');if(!n.label||!n.role||!n.workInstruction||!['scholar','scribe','artist','analyst','messenger'].includes(n.spriteKey))errors.push('工程名・役割・指示・キャラクターを確認してください。');if(!['codex','manual','save','quality'].includes(n.executor))errors.push('実行方法が正しくありません。');if(!Array.isArray(n.dependencies)||!Array.isArray(n.inputArtifacts)||!Array.isArray(n.outputArtifacts))errors.push('dependencies/inputArtifacts/outputArtifacts は配列が必要です。');else if(n.outputArtifacts.some(p=>!p||/[\\/:]/.test(p)||p==='..'||p.includes('..')))errors.push('出力名はフォルダを含まないファイル名にしてください。');}
 if(errors.length)return [...new Set(errors)];
 for(const e of w.edges){if(!e.edgeId||edges.has(e.edgeId))errors.push('接続IDが空、または重複しています。');edges.add(e.edgeId);if(!ids.has(e.fromNodeId)||!ids.has(e.toNodeId))errors.push('存在しない工程への接続があります。');}
 for(const n of w.nodes){const incoming=w.edges.filter(e=>e.toNodeId===n.nodeId).map(e=>e.fromNodeId).sort();if(new Set(n.dependencies).size!==n.dependencies.length||JSON.stringify([...n.dependencies].sort())!==JSON.stringify(incoming))errors.push(`${n.label}：依存関係と接続線が一致しません。`);}
 const visiting=new Set<string>(),done=new Set<string>();let cycle=false;
 const visit=(id:string)=>{if(visiting.has(id)){cycle=true;return;}if(done.has(id))return;visiting.add(id);for(const e of w.edges.filter(e=>e.fromNodeId===id))visit(e.toNodeId);visiting.delete(id);done.add(id);};
 for(const id of ids)visit(id);if(cycle)errors.push('依存関係が循環しています。');return [...new Set(errors)];
}
export function layoutWorkflow(w:Workflow){const g=new dagre.graphlib.Graph();g.setGraph({rankdir:'LR',nodesep:70,ranksep:48,marginx:28,marginy:76});g.setDefaultEdgeLabel(()=>({}));for(const n of w.nodes)g.setNode(n.nodeId,{width:136,height:184});for(const e of w.edges)g.setEdge(e.fromNodeId,e.toNodeId);dagre.layout(g);return Object.fromEntries(w.nodes.map(n=>[n.nodeId,{x:g.node(n.nodeId).x-68,y:g.node(n.nodeId).y-92}]));}
