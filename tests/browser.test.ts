import {describe,it,expect} from 'vitest';
import {browsers,desktopBrowser,handoffLabel,canRedisplay,displayLabel,type HandoffSummary} from '../src/core/browser';
describe('Core browser contract',()=>{
 it('keeps all choices while limiting verified adapter scope',()=>{expect(browsers.map(b=>b.id)).toEqual(['iab','chrome','edge','brave','opera','vivaldi','system']);expect(browsers.filter(b=>desktopBrowser(b.id)).map(b=>b.id)).toEqual(['iab','chrome']);});
 it('does not display OS dispatch as a verified send or publication',()=>{expect(handoffLabel('DESKTOP_OS_ACCEPTED_USER_ACTION_REQUIRED')).toContain('送信は未確認');expect(handoffLabel('DESKTOP_OPEN_REQUESTED_UNKNOWN')).toContain('UNKNOWN');expect(handoffLabel('unrecognized')).toContain('UNKNOWN');});
 it('enables only explicit resolved redisplays and keeps unknown requests stopped',()=>{
  const h={status:'DESKTOP_OS_ACCEPTED_USER_ACTION_REQUIRED',desktopDisplays:[]} as unknown as HandoffSummary;
  expect(canRedisplay(h)).toBe(true);expect(canRedisplay({...h,status:'DESKTOP_OPEN_FAILED'})).toBe(true);
  for(const status of ['HANDOFF_PREPARED','DESKTOP_OPEN_REQUESTED_UNKNOWN','PUBLISHED_VERIFIED'])expect(canRedisplay({...h,status})).toBe(false);
  expect(canRedisplay({...h,desktopDisplays:[{osStatus:'REQUESTED_UNKNOWN'} as never]})).toBe(false);
  expect(displayLabel('OS_ACCEPTED')).toContain('入力欄は未確認');expect(displayLabel('OS_REJECTED')).toContain('手動コピー');
 });
});
