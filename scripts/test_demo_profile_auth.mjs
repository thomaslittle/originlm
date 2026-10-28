import {test} from 'node:test';
import assert from 'node:assert/strict';
import {connectionAuthLabel} from '../apps/demo/lib/profile_auth.mjs';

test('connection labels distinguish verified, stale, unchecked and local non-account runtimes',()=>{
 assert.equal(connectionAuthLabel({auth:{state:'known',value:'authenticated',freshness:'fresh'}}),'Authenticated');
 assert.equal(connectionAuthLabel({auth:{state:'known',value:'authenticated',freshness:'stale'}}),'Previously authenticated');
 assert.equal(connectionAuthLabel({auth:{state:'unknown',reason:'LOCAL_RUNTIME_HAS_NO_ACCOUNT_AUTH'}}),'Local runtime · account auth not applicable');
 assert.equal(connectionAuthLabel({auth:{state:'unknown',reason:'OPENCODE_UPSTREAM_CREDENTIALS_NOT_VALIDATED'}}),'Credentials configured · authentication not verified');
 assert.equal(connectionAuthLabel({auth:{state:'unknown',reason:'NOT_CHECKED'}}),'Not checked');
});
