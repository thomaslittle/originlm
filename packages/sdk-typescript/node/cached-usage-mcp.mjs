/** Optional bounded stdio framing. Import creates no process, connection, timer or listener. */
import {once} from 'node:events';
const MAX_INPUT=64*1024, MAX_OUTPUT=512*1024;
export async function serveCachedUsageMcp(handler,{input=process.stdin,output=process.stdout}={}) {
 let pending=Buffer.alloc(0);
 const write=async value=>{
  if(value===undefined)return;
  const frame=Buffer.from(JSON.stringify(value)+'\n');
  if(frame.length>MAX_OUTPUT)throw new Error('MCP_OUTPUT_LIMIT');
  if(!output.write(frame))await once(output,'drain');
 };
 for await(const chunk of input){
  pending=Buffer.concat([pending,Buffer.from(chunk)]);
  let newline;
  while((newline=pending.indexOf(10))>=0){
   if(newline>MAX_INPUT)throw new Error('MCP_INPUT_LIMIT');
   const frame=pending.subarray(0,newline);pending=pending.subarray(newline+1);
   if(!frame.length)continue;
   let packet;
   try{packet=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(frame));}
   catch{await write({jsonrpc:'2.0',id:null,error:{code:-32700,message:'Invalid MCP JSON'}});continue;}
   await write(handler(packet));
  }
  if(pending.length>MAX_INPUT)throw new Error('MCP_INPUT_LIMIT');
 }
 if(pending.length)await write({jsonrpc:'2.0',id:null,error:{code:-32600,message:'Incomplete MCP frame'}});
}
