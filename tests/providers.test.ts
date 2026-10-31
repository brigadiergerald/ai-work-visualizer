import {describe,it,expect} from 'vitest';
import {canSaveTargets,canTestConnection,type ProviderConnection} from '../src/core/providers';
describe('provider connection controls',()=>{
 it('separates configuration, authentication, targets and unknown',()=>{
  const c=(state:string)=>({state} as ProviderConnection);
  expect(canSaveTargets(c('CONFIGURED'))).toBe(false);expect(canSaveTargets(c('AUTHENTICATED'))).toBe(false);
  expect(canSaveTargets(c('TARGETS_VERIFIED'))).toBe(true);expect(canSaveTargets(c('MOCK_VERIFIED'))).toBe(true);
  for(const s of ['UNKNOWN','CHECKING_UNKNOWN','DISCONNECTED']){expect(canTestConnection(c(s))).toBe(false);expect(canSaveTargets(c(s))).toBe(false);}
 });
});
