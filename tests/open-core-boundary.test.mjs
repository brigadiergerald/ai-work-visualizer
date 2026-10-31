import {test as it} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,readdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
it('keeps dashboard free of provider protocols and legacy material imports',()=>{
 const ui=readFileSync('src/dashboard/FreeDashboard.tsx','utf8');for(const s of ['requestUserInput','requestId','turn/start','threadId',"from '../model'","from '../store'",'spriteKey'])assert.ok(!ui.includes(s));
 assert.ok(readFileSync('src/core/client.ts','utf8').includes('core:snapshot'));assert.ok(!readFileSync('src/main.tsx','utf8').includes("'./App'"));
});
it('production frontend allows only the five SHA-pinned local sprites and excludes dev fixtures, fonts and legacy UI',()=>{
 const base=process.env.OPEN_CORE_FRONTEND??'dist-open-core',files=readdirSync(`${base}/assets`);
 assert.equal(files.some(f=>/\.(woff2?|ttf)$/i.test(f)),false);
 const manifest=JSON.parse(readFileSync('src/assets/work-people-v1/manifest.json','utf8'));
 const js=files.filter(f=>f.endsWith('.js')).map(f=>readFileSync(`${base}/assets/${f}`,'utf8')).join('');
 const pngs=files.filter(f=>f.endsWith('.png'));
 const actual=pngs.map(f=>createHash('sha256').update(readFileSync(`${base}/assets/${f}`)).digest('hex'));
 for(const match of js.matchAll(/data:image\/png;base64,([A-Za-z0-9+/=]+)/g))actual.push(createHash('sha256').update(Buffer.from(match[1],'base64')).digest('hex'));
 assert.deepEqual([...new Set(actual)].sort(),manifest.characters.map(c=>c.sha256).sort());
 for(const text of ['OFFLINE FIXTURE:','S3_NATIVE_FIXTURE','/sprites/','RPG','リオ','turn/start','requestUserInput'])assert.ok(!js.includes(text),text);
 for(const text of ['AI GUILD','AI Guild','AI Office'])assert.ok(!js.includes(text),text);
 for(const f of ['StageScene.tsx','StageActor.tsx','StageConnections.tsx','stage-visuals.ts','stage-assets.ts']){
  const source=readFileSync(`src/dashboard/${f}`,'utf8');
  for(const forbidden of ['core/client','@tauri-apps','invoke(','execute(','localStorage','fetch(','http://','https://'])assert.ok(!source.includes(forbidden),`${f}: ${forbidden}`);
 }
});
it('ties UI fixture content to the actual saved artifact hash',()=>{
 const fixture=JSON.parse(readFileSync('tests/fixtures/open-core.json','utf8'));assert.equal(fixture.text.sha256,createHash('sha256').update(fixture.text.text).digest('hex'));
});
