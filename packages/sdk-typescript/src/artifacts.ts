import type {LocalAIClient} from './client.ts';
import {LocalAIError} from './errors.ts';
export interface ArtifactBytes {bytes:Uint8Array;displayName:string;mediaType:string}
const invalid=()=>new LocalAIError('PROTOCOL_INVALID');
function decimal(value:unknown):number {
 if(typeof value!=='string'||!/^(0|[1-9][0-9]{0,8})$/.test(value))throw invalid();
 const number=Number(value);if(number>4*1024*1024)throw invalid();return number;
}
/** Explicit, bounded download of registered bytes. It cannot read arbitrary native paths. */
export async function readArtifactBytes(client:LocalAIClient,sessionId:string,artifactId:string):Promise<ArtifactBytes>{
 if(!sessionId||!artifactId||sessionId.length>256||artifactId.length>256)throw new LocalAIError('INVALID_REQUEST');
 let offset=0,total:number|undefined,bytes:Uint8Array|undefined,displayName='',mediaType='';
 for(let n=0;n<128;n++){
  const chunk=await client.artifacts.read({sessionId,artifactId,offset:String(offset),maxBytes:49152});
  if(!chunk||chunk.sessionId!==sessionId||chunk.artifactId!==artifactId||decimal(chunk.offset)!==offset||typeof chunk.dataBase64!=='string'||chunk.dataBase64.length>65536||typeof chunk.eof!=='boolean')throw invalid();
  const size=decimal(chunk.totalBytes),next=decimal(chunk.nextOffset);
  if(total===undefined){
   if(typeof chunk.displayName!=='string'||!chunk.displayName||chunk.displayName.length>128||typeof chunk.mediaType!=='string'||chunk.mediaType.length>128)throw invalid();
   total=size;bytes=new Uint8Array(size);displayName=chunk.displayName;mediaType=chunk.mediaType;
  }
  if(size!==total||chunk.displayName!==displayName||chunk.mediaType!==mediaType||next<offset||next>total)throw invalid();
  if(!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(chunk.dataBase64))throw invalid();
  let raw:string;try{raw=atob(chunk.dataBase64);}catch{throw invalid();}
  if(raw.length!==next-offset||raw.length>49152||(!chunk.eof&&next===offset))throw invalid();
  bytes!.set(Uint8Array.from(raw,c=>c.charCodeAt(0)),offset);offset=next;
  if(chunk.eof){if(offset!==total)throw invalid();return {bytes:bytes!,displayName,mediaType};}
  if(offset===total)throw invalid();
 }
 throw new LocalAIError('FRAME_TOO_LARGE');
}
