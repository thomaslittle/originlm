import {test} from 'node:test';
import assert from 'node:assert/strict';
import {debugSessionError,userFacingError,userFacingSessionError} from '../apps/demo/lib/errors.mjs';

test('provider capability failures are readable and never expose raw codes',()=>{
 assert.equal(userFacingError({code:'CAPABILITY_UNSUPPORTED'}),'This provider cannot perform that operation in the current runtime.');
 assert.equal(userFacingError({code:'RUNTIME_MISSING'}),'A compatible native provider runtime is not installed or registered.');
 assert.match(userFacingError({code:'LOCAL_ACCESS_DENIED'}),/local access required for the request/);
 assert.equal(userFacingError({code:'SOME_NEW_INTERNAL_CODE'}),'The request could not be completed.');
});

test('session failures explain normalized codes and ignore provider supplied messages',()=>{
 const detail=userFacingSessionError({code:'PROVIDER_UNAVAILABLE',message:'PRIVATE_PROVIDER_DETAIL'});
 assert.match(detail,/provider client stopped or returned an unavailable response/i);
 assert.match(detail,/Error code: PROVIDER_UNAVAILABLE/);
 assert.doesNotMatch(detail,/PRIVATE_PROVIDER_DETAIL/);
 assert.match(userFacingSessionError({code:'AUTH_REQUIRED'}),/native provider client/i);
 assert.match(userFacingSessionError({code:'SOME_NEW_INTERNAL_CODE',message:'PRIVATE'}),/did not provide a recognized error code/i);
 assert.doesNotMatch(userFacingSessionError({code:'SOME_NEW_INTERNAL_CODE',message:'PRIVATE'}),/PRIVATE|SOME_NEW_INTERNAL_CODE/);
});

test('debug details retain provider diagnostics while redacting credentials and bounding output',()=>{
 const detail=debugSessionError({code:'PROVIDER_UNAVAILABLE',message:'HTTP 401: Authorization: Bearer private-token-value'});
 assert.equal(detail.code,'PROVIDER_UNAVAILABLE');
 assert.equal(detail.message,'HTTP 401: Authorization: Bearer [REDACTED]');
 const json=debugSessionError({code:'AUTH_REQUIRED',message:'{"access_token":"secret-value"}'});
 assert.equal(json.message,'{"access_token":"[REDACTED]"}');
 assert.ok(debugSessionError({message:'x'.repeat(5000)}).message.length<=2048);
});
