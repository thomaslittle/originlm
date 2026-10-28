import {test} from 'node:test';
import assert from 'node:assert/strict';

test('OpenCode live-demo registration keeps its generated password out of the endpoint file',async()=>{
 const {buildOpenCodeRegistration}=await import('./live_demo_opencode.mjs');
 const secret='runtime-only-secret-runtime-only-secret';
 const registration=buildOpenCodeRegistration(43127,secret);
 assert.equal(registration.endpoint.baseUrl,'http://127.0.0.1:43127/');
 assert.equal(registration.endpoint.keyEnv,'LOCAL_AI_KIT_OPENCODE_PASSWORD');
 assert.equal(JSON.stringify(registration.endpoint).includes(secret),false);
 assert.equal(registration.environment.LOCAL_AI_KIT_OPENCODE_PASSWORD,secret);
 assert.equal(registration.serverEnvironment.OPENCODE_SERVER_PASSWORD,secret);
 assert.equal(registration.serverEnvironment.OPENCODE_SERVER_USERNAME,'opencode');
 assert.equal(registration.environment.OPENCODE_SERVER_PASSWORD,undefined);
 assert.deepEqual(registration.serverArgs,['serve','--port','43127','--hostname','127.0.0.1','--pure']);
});

test('server readiness requires both accepted credentials and rejected anonymous access',async()=>{
 const {verifyOpenCodeAuthentication}=await import('./live_demo_opencode.mjs');
 const calls=[];
 const request=async(_url,options)=>{
  calls.push(options);
  return {ok:!!options.headers?.Authorization,status:options.headers?.Authorization?200:401};
 };
 assert.equal(await verifyOpenCodeAuthentication('http://127.0.0.1:43127/','secret',request),true);
 assert.equal(calls.length,2);
 assert.equal(await verifyOpenCodeAuthentication('http://127.0.0.1:43127/','secret',async()=>({ok:true,status:200})),false);
 assert.equal(await verifyOpenCodeAuthentication('http://127.0.0.1:43127/','secret',async()=>({ok:false,status:401})),false);
});
