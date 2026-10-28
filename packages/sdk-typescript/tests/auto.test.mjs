import test from 'node:test';
import assert from 'node:assert/strict';
import {chmodSync, cpSync, mkdirSync, readFileSync, realpathSync, mkdtempSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {openSharedBrokerTransport} from '../node/index.mjs';
import {readGrant} from '../node/grant.mjs';
import {defaultAddress, defaultTokenFile, platformPackage, resolveHelper, openAutoTransport} from '../node/auto.mjs';

function helper() {
  const directory = mkdtempSync(join(tmpdir(), 'originlm-helper-'));
  const binary = join(directory, 'originlm');
  writeFileSync(binary, '#!/bin/sh\n');
  chmodSync(binary, 0o755);
  return {directory, binary};
}

test('platform packages mirror esbuild and Biome', () => {
  assert.equal(platformPackage('linux', 'x64'), '@originlm/cli-linux-x64');
  assert.equal(platformPackage('linux', 'arm64'), '@originlm/cli-linux-arm64');
  assert.equal(platformPackage('darwin', 'arm64'), '@originlm/cli-darwin-arm64');
  assert.equal(platformPackage('darwin', 'x64'), '@originlm/cli-darwin-x64');
  assert.equal(platformPackage('win32', 'x64'), '@originlm/cli-win32-x64');
  assert.equal(platformPackage('freebsd', 'x64'), undefined);
});

test('LAI_HELPER_PATH overrides packaged resolution', () => {
  const {directory, binary} = helper();
  const previous = process.env.LAI_HELPER_PATH;
  process.env.LAI_HELPER_PATH = binary;
  try {
    assert.equal(resolveHelper(), binary);
  } finally {
    if (previous === undefined) delete process.env.LAI_HELPER_PATH;
    else process.env.LAI_HELPER_PATH = previous;
    rmSync(directory, {recursive: true, force: true});
  }
});

test('a bad override fails loudly instead of falling back', () => {
  const previous = process.env.LAI_HELPER_PATH;
  process.env.LAI_HELPER_PATH = '/nonexistent/originlm';
  try {
    assert.throws(() => resolveHelper(), /INVALID_EXECUTABLE/);
  } finally {
    if (previous === undefined) delete process.env.LAI_HELPER_PATH;
    else process.env.LAI_HELPER_PATH = previous;
  }
  assert.throws(() => resolveHelper({executable: 'relative/helper'}), /INVALID_EXECUTABLE/);
});

test('no binary and no override is a runtime-missing error', () => {
  const previous = process.env.LAI_HELPER_PATH;
  delete process.env.LAI_HELPER_PATH;
  try {
    assert.throws(() => resolveHelper(), /RUNTIME_MISSING/);
  } finally {
    if (previous !== undefined) process.env.LAI_HELPER_PATH = previous;
  }
});

test('shared broker defaults follow the environment', () => {
  const previous = {
    address: process.env.LAI_BROKER_ADDRESS,
    token: process.env.LAI_BROKER_TOKEN_FILE,
  };
  process.env.LAI_BROKER_ADDRESS = '/tmp/custom.sock';
  process.env.LAI_BROKER_TOKEN_FILE = '/tmp/custom-token.json';
  try {
    assert.equal(defaultAddress(), '/tmp/custom.sock');
    assert.equal(defaultTokenFile(), '/tmp/custom-token.json');
  } finally {
    if (previous.address === undefined) delete process.env.LAI_BROKER_ADDRESS;
    else process.env.LAI_BROKER_ADDRESS = previous.address;
    if (previous.token === undefined) delete process.env.LAI_BROKER_TOKEN_FILE;
    else process.env.LAI_BROKER_TOKEN_FILE = previous.token;
  }
  assert.match(defaultTokenFile(), /broker-token\.json$/);
});


test('malformed private startup reaps the child before rejecting', {skip: process.platform === 'win32'}, async () => {
  const {directory, binary} = helper();
  const pidFile = join(directory, 'pid');
  writeFileSync(binary, '#!/bin/sh\nprintf "%s" "$$" > "$LAI_TEST_PID"\nprintf "invalid\\n"\nwhile read -r line; do :; done\n');
  try {
    await assert.rejects(openAutoTransport({executable: binary, token: '', env: {LAI_TEST_PID: pidFile}}), /PROTOCOL_INVALID/);
    const pid = Number(readFileSync(pidFile, 'utf8'));
    assert.throws(() => process.kill(pid, 0), {code: 'ESRCH'});
  } finally {
    rmSync(directory, {recursive: true, force: true});
  }
});

test('protocol mismatch falls back but grant denial never does', {skip: process.platform === 'win32'}, async () => {
  const {directory, binary} = helper();
  writeFileSync(binary, `#!/usr/bin/env node
import('node:readline').then(({createInterface}) => {
  const address = process.argv[process.argv.indexOf('--ipc') + 1];
  const serving = process.argv[2] === 'serve';
  if (serving) process.stdout.write(JSON.stringify({protocolVersion:'1.0',address,token:'a'.repeat(64)})+'\\n');
  const lines = createInterface({input:process.stdin});
  lines.on('close',()=>process.exit(0));
  lines.on('line',line=>{
    if (serving) return;
    const request=JSON.parse(line);
    const denied=address==='/fixture/denied';
    const result=request.method==='broker.authenticate' ? {authenticated:true} : {protocolVersion:address==='/fixture/shared'?'2.0':'1.0',methods:[]};
    process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:request.id,...(denied?{error:{code:-32000,data:{code:'LOCAL_ACCESS_DENIED'}}}:{result})})+'\\n');
  });
});
`);
  try {
    const transport = await openAutoTransport({executable:binary,address:'/fixture/shared',token:'a'.repeat(64),passive:false});
    assert.equal(transport.mode,'child');
    assert.equal(transport.fallbackReason, 'FALLBACK_PROTOCOL_MISMATCH');
    await transport.close();
    await assert.rejects(openAutoTransport({executable:binary,address:'/fixture/denied',token:'a'.repeat(64)}), error=>error.code==='LOCAL_ACCESS_DENIED');
  } finally { rmSync(directory,{recursive:true,force:true}); }
});

test('non-executable Unix overrides fail before spawning', {skip: process.platform === 'win32'}, () => {
  const {directory, binary} = helper();
  try {
    chmodSync(binary, 0o600);
    assert.throws(() => resolveHelper({executable: binary}), /INVALID_EXECUTABLE/);
  } finally { rmSync(directory, {recursive: true, force: true}); }
});

test('private broker receives only the approved environment and caller cwd', {skip: process.platform === 'win32'}, async () => {
  const {directory, binary} = helper();
  const observed = join(directory, 'observed');
  const previous = process.env.UNRELATED_SYNTHETIC_SECRET;
  process.env.UNRELATED_SYNTHETIC_SECRET = 'must-not-inherit';
  writeFileSync(binary, '#!/bin/sh\nprintf "%s|%s|%s" "$PWD" "$EXPLICIT_VALUE" "$UNRELATED_SYNTHETIC_SECRET" > "$LAI_TEST_OUTPUT"\nprintf "invalid\\n"\nwhile read -r line; do :; done\n');
  try {
    await assert.rejects(openAutoTransport({executable: binary, token: '', cwd: directory, env: {LAI_TEST_OUTPUT: observed, EXPLICIT_VALUE: 'allowed'}}), /PROTOCOL_INVALID/);
    assert.equal(readFileSync(observed, 'utf8'), `${realpathSync(directory)}|allowed|`);
  } finally {
    if (previous === undefined) delete process.env.UNRELATED_SYNTHETIC_SECRET;
    else process.env.UNRELATED_SYNTHETIC_SECRET = previous;
    rmSync(directory, {recursive: true, force: true});
  }
});

test('grant reader rejects oversized descriptors', () => {
  const {directory} = helper();
  const file = join(directory, 'grant.json');
  try {
    writeFileSync(file, JSON.stringify({address:'/fixture/shared',token:'a'.repeat(64),padding:'x'.repeat(16384)}), {mode:0o600});
    assert.equal(readGrant(file, '/fixture/shared'), null);
  } finally { rmSync(directory, {recursive:true, force:true}); }
});

test('shared startup never hides an authentication denial', {skip: process.platform === 'win32'}, async () => {
  const {directory, binary} = helper();
  const file = join(directory, 'grant.json');
  const marker = join(directory, 'started');
  writeFileSync(file, JSON.stringify({protocolVersion:'1.0',address:'/fixture/shared',token:'a'.repeat(64)}), {mode:0o600});
  writeFileSync(binary, `#!/usr/bin/env node
import('node:readline').then(({createInterface}) => {
  if(process.argv[2]==='serve') import('node:fs').then(fs=>fs.writeFileSync(process.env.START_MARKER,'unexpected'));
  const lines=createInterface({input:process.stdin});
  lines.on('line',line=>{const request=JSON.parse(line);process.stdout.write(JSON.stringify({jsonrpc:'2.0',id:request.id,error:{code:-32000,data:{code:'LOCAL_ACCESS_DENIED'}}})+'\\n');});
});
`);
  try {
    await assert.rejects(openSharedBrokerTransport({executable:binary,address:'/fixture/shared',tokenFile:file,env:{START_MARKER:marker}}), error=>error.code==='LOCAL_ACCESS_DENIED');
    assert.throws(()=>readFileSync(marker), {code:'ENOENT'});
  } finally { rmSync(directory,{recursive:true,force:true}); }
});

test('Electron packaged helpers resolve to the real unpacked tree',async()=>{
 const directory=mkdtempSync(join(tmpdir(),'originlm-asar-'));
 const name=platformPackage();if(!name){rmSync(directory,{recursive:true,force:true});return;}
 const modules=join(directory,'app.asar','node_modules');
 const sdk=join(modules,'@originlm','sdk');mkdirSync(sdk,{recursive:true});
 cpSync(fileURLToPath(new URL('../node',import.meta.url)),join(sdk,'node'),{recursive:true});
 cpSync(fileURLToPath(new URL('../dist',import.meta.url)),join(sdk,'dist'),{recursive:true});
 writeFileSync(join(sdk,'package.json'),JSON.stringify({type:'module'}));
 const platform=join(modules,name);mkdirSync(platform,{recursive:true});
 const binary=process.platform==='win32'?'originlm.exe':'originlm';
 writeFileSync(join(platform,'package.json'),JSON.stringify({name,bin:{'originlm':`bin/${binary}`}}));
 const unpacked=join(directory,'app.asar.unpacked','node_modules',name,'bin',binary);
 mkdirSync(join(unpacked,'..'),{recursive:true});writeFileSync(unpacked,'fixture');chmodSync(unpacked,0o755);
 const descriptor=Object.getOwnPropertyDescriptor(process.versions,'electron');
 const previous=process.env.LAI_HELPER_PATH;delete process.env.LAI_HELPER_PATH;
 try {
  const isolated=await import(pathToFileURL(join(sdk,'node','auto.mjs')).href);
  assert.throws(()=>isolated.resolveHelper(),/RUNTIME_MISSING/);
  Object.defineProperty(process.versions,'electron',{value:'fixture',configurable:true});
  assert.equal(isolated.resolveHelper(),realpathSync(unpacked));
  rmSync(unpacked);assert.throws(()=>isolated.resolveHelper(),/HELPER_NOT_UNPACKED/);
 }finally{
  if(descriptor)Object.defineProperty(process.versions,'electron',descriptor);else delete process.versions.electron;
  if(previous===undefined)delete process.env.LAI_HELPER_PATH;else process.env.LAI_HELPER_PATH=previous;
  rmSync(directory,{recursive:true,force:true});
 }
});
