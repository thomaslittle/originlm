/** Explicit local-bundle validation. Checksums are integrity checks, not signatures. */
import {lstat,open} from 'node:fs/promises';
import {isAbsolute,join} from 'node:path';
import {createHash} from 'node:crypto';
import {LocalAIError} from '../dist/errors.js';
const invalid=()=>new LocalAIError('BUNDLE_INVALID');
const targets={win32:{x64:['x86_64-pc-windows-msvc'],arm64:['aarch64-pc-windows-msvc']},linux:{x64:['x86_64-unknown-linux-gnu','x86_64-unknown-linux-musl'],arm64:['aarch64-unknown-linux-gnu','aarch64-unknown-linux-musl']},darwin:{x64:['x86_64-apple-darwin'],arm64:['aarch64-apple-darwin']}};
async function regular(path){const stat=await lstat(path);if(!stat.isFile()||stat.isSymbolicLink())throw invalid();return stat;}
export async function resolveBundledBinary(directory){
 try {
  if(typeof directory!=='string'||!isAbsolute(directory)||directory.includes('\0'))throw invalid();
  for(const path of [directory,join(directory,'bin')]){const s=await lstat(path);if(!s.isDirectory()||s.isSymbolicLink())throw invalid();}
  const manifestPath=join(directory,'manifest.json');const stat=await regular(manifestPath);if(stat.size>1048576)throw invalid();
  const file=await open(manifestPath,'r');let bytes;
  try {const buffer=Buffer.alloc(1048577);let size=0;while(size<buffer.length){const r=await file.read(buffer,size,buffer.length-size,null);if(!r.bytesRead)break;size+=r.bytesRead;}if(size>1048576)throw invalid();bytes=buffer.subarray(0,size);}finally{await file.close();}
  const manifest=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));
  if(!manifest||manifest.schemaVersion!==1||typeof manifest.version!=='string'||manifest.version.length>64||!manifest.files)throw invalid();
  if(!targets[process.platform]?.[process.arch]?.includes(manifest.platform))throw new LocalAIError('PLATFORM_UNSUPPORTED');
  const name='bin/originlm'+(process.platform==='win32'?'.exe':'');const record=manifest.files[name];
  if(!record||!Number.isSafeInteger(record.bytes)||record.bytes<=0||record.bytes>1073741824||!/^[0-9a-f]{64}$/.test(record.sha256))throw invalid();
  const binary=join(directory,name);const before=await regular(binary);if(before.size!==record.bytes)throw invalid();
  const handle=await open(binary,'r');try{
   const now=await handle.stat();if(now.ino!==before.ino||now.dev!==before.dev||now.size!==record.bytes)throw invalid();
   const hash=createHash('sha256');let size=0;
   for await(const chunk of handle.createReadStream({autoClose:false})){size+=chunk.length;if(size>record.bytes)throw invalid();hash.update(chunk);}
   if(size!==record.bytes||hash.digest('hex')!==record.sha256)throw invalid();
  }finally{await handle.close();}
  return binary;
 }catch(error){throw error instanceof LocalAIError?error:invalid();}
}
