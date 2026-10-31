import planner from '../assets/work-people-v1/planner.png?url';
import writer from '../assets/work-people-v1/writer.png?url';
import verifier from '../assets/work-people-v1/verifier.png?url';
import reviewer from '../assets/work-people-v1/reviewer.png?url';
import archivist from '../assets/work-people-v1/archivist.png?url';

/** Trusted built-in visual data only. No URL, script, CSS or package input. */
export const spriteSpec={cellWidth:48,cellHeight:72,columns:4,rows:6,scale:2} as const;
export const builtInPeople:Readonly<Record<string,{label:string;sprite:string;prop:string}>>={
 researcher:{label:'調査',sprite:planner,prop:'books'},
 director:{label:'ディレクター',sprite:reviewer,prop:'lamp'},
 creator:{label:'製作',sprite:writer,prop:'paper'},
 inspector:{label:'検品・投稿',sprite:verifier,prop:'check'},
 planner:{label:'計画',sprite:planner,prop:'books'},
 writer:{label:'文章制作',sprite:writer,prop:'paper'},
 verifier:{label:'品質確認',sprite:verifier,prop:'check'},
 reviewer:{label:'内容確認・承認',sprite:reviewer,prop:'lamp'},
 archivist:{label:'保存・納品',sprite:archivist,prop:'cabinet'}
};
