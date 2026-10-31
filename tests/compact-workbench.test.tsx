import {expect,it} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import StageScene from '../src/dashboard/StageScene';
import {idleStations} from '../src/dashboard/CompactStageRoom';
import {stationsFor} from '../src/dashboard/stage-visuals';
import {compactSkinVisual} from '../src/dashboard/skins/skin-renderer';
import type {PresentationSnapshot} from '../src/core/presentation';
import type {RenderSkin} from '../src/core/skins';
import frame from '../tests/fixtures/sns-presentation-completed.json';
import blue from '../tests/fixtures/render-skin.json';

it('keeps all four roles before a Job starts and distinguishes missing live evidence from idle',()=>{
 const stations=idleStations('social-text');
 expect(stations.map(s=>s.role.id)).toEqual(['researcher','director','creator','inspector']);
 expect(stations.every(s=>s.task===null&&!s.moving&&s.status==='IDLE')).toBe(true);
 expect(stations[3].role.actor).toBe('host');
 const idle=renderToStaticMarkup(<StageScene compact purpose="social-text" snapshot={null} hasJob={false} visible handoffs={[]}/>);
 expect((idle.match(/class="stage-station /g)??[]).length).toBe(4);
 const loading=renderToStaticMarkup(<StageScene compact roleMapVersion="social-text-roles-v2" snapshot={null} hasJob visible handoffs={[]}/>);
 expect((loading.match(/class="stage-station stage-unknown/g)??[]).length).toBe(4);
 expect(loading).not.toContain('is-working');
});
it('preserves each persisted role even when a skin assigns the same person to multiple roles',()=>{
 const snapshot=structuredClone(frame) as PresentationSnapshot;
 const skin=structuredClone(blue) as unknown as RenderSkin;
 skin.layout.assignments.forEach(a=>{(a as {characterId:string}).characterId=skin.layout.characters[0].characterId;});
 const original=JSON.stringify(snapshot);
 for(const station of stationsFor(snapshot))expect(compactSkinVisual(skin,station,1)?.width).toBeGreaterThan(0);
 const html=renderToStaticMarkup(<StageScene compact snapshot={snapshot} skin={skin} hasJob visible handoffs={[]}/>);
 expect((html.match(/class="stage-station /g)??[]).length).toBe(snapshot.roles.length);
 for(const role of snapshot.roles)expect(html).toContain('data-role="'+role.id+'"');
 expect(JSON.stringify(snapshot)).toBe(original);
});
