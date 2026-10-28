// Synthetic protocol peer only. Never starts a provider or reads user state.
import { createInterface } from 'node:readline';
const write=value=>process.stdout.write(JSON.stringify(value)+'\n');
const result=(id,value)=>write({jsonrpc:'2.0',id,result:value});
createInterface({input:process.stdin,crlfDelay:Infinity}).on('line',line=>{
 const r=JSON.parse(line);
 if(r.method==='runtime.handshake')result(r.id,{protocolVersion:'1.0',methods:['snapshot.get','events.subscribe','events.unsubscribe']});
 else if(r.method==='test.exit')process.exit(0);
 else if(r.method==='test.hang'){}
 else if(r.method==='test.bad')process.stdout.write('not-json\n');
 else if(r.method==='test.large')process.stdout.write('x'.repeat(2048));
 else if(r.method==='test.stderr'){process.stderr.write('SECRET_SHOULD_NOT_ESCAPE');result(r.id,{ok:true});}
 else if(r.method==='test.directories')result(r.id,Object.fromEntries(['XDG_STATE_HOME','APPDATA','LOCALAPPDATA','LOCAL_AI_KIT_STATE_DIR','ORIGINLM_STATE_DIR'].map(key=>[key,process.env[key]??null])));
 else if(r.method==='test.env')result(r.id,{nodeOptions:process.env.NODE_OPTIONS??null,custom:process.env.EXPLICIT_VALUE??null});
 else if(r.method==='test.gap'){write({jsonrpc:'2.0',method:'events.error',params:{subscriptionId:'synthetic-sub',code:'EVENT_GAP'}});result(r.id,{ok:true});}
 else if(r.method==='test.flood'){
  for(let i=0;i<20;i++)write({jsonrpc:'2.0',method:'events.event',params:{subscriptionId:'synthetic-sub',event:{sequence:String(i+2)}}});result(r.id,{sent:true});
 }
 else if(r.method==='events.subscribe'){
  result(r.id,{subscriptionId:'synthetic-sub'});
  for(let i=0;i<(r.params.after==='manual'?1:3);i++)write({jsonrpc:'2.0',method:'events.event',params:{subscriptionId:'synthetic-sub',event:{sequence:String(i+1),text:'synthetic-'+i}}});
 }
 else if(r.method==='events.unsubscribe')result(r.id,{ok:true});
 else if(r.method==='test.delay')setTimeout(()=>result(r.id,r.params.value),r.params.delay);
 else result(r.id,r.params);
});
