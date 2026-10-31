import {describe,it,expect} from 'vitest';
import {themes,loadTheme,chooseTheme,themeStyle} from '../src/themes/theme';
import fixture from './fixtures/open-core.json';
const digest=(v:unknown)=>JSON.stringify(v);
describe('OPEN-CORE theme isolation',()=>{
 it('switches two declarative layouts without touching jobs, approvals, files, settings or hashes',()=>{
  const before=digest(fixture);for(const t of themes){const next=loadTheme(t);expect(chooseTheme(t.id)).toEqual(next);expect(Object.keys(themeStyle(next))).toHaveLength(7);}expect(chooseTheme('missing').id).toBe('free-standard');expect(digest(fixture)).toBe(before);
 });
 it('rejects active content, extra fields, remote assets, fonts, raw CSS, missing sections and invented states',()=>{
  for(const mutate of [(v:any)=>v.script='alert(1)',(v:any)=>v.colors.accent='url(https://example.test)',(v:any)=>v.font='https://example.test/font',(v:any)=>v.assets.character='../private.png',(v:any)=>v.layout.sections=['compose'],(v:any)=>v.animations.imaginary='pulse',(v:any)=>v.animations.running='arbitrary-code']){const v=structuredClone(themes[0]);mutate(v);expect(()=>loadTheme(v)).toThrow();}
 });
 it('uses actual normalized offline snapshots and preserves approval correspondence',()=>{
  const a=fixture.awaiting.jobs[0],b=fixture.completed.jobs[0];expect(a.id).toBe(b.id);expect(a.state).toBe('awaiting_approval');expect(b.state).toBe('completed');expect(a.publication).toBe('NOT_PERFORMED');expect(fixture.text.sha256).toBe(a.artifacts.find(v=>v.name==='post.txt')?.sha256);
 });
});
