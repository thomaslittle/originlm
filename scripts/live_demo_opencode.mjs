const passwordVariable='LOCAL_AI_KIT_OPENCODE_PASSWORD';

export function buildOpenCodeRegistration(port,password){
 if(!Number.isInteger(port)||port<1||port>65535||typeof password!=='string'||password.length<32)throw new Error('INVALID_OPENCODE_REGISTRATION');
 return {
  endpoint:{id:'live-demo-opencode',label:'OpenCode local',kind:'opencode',baseUrl:`http://127.0.0.1:${port}/`,keyEnv:passwordVariable},
  environment:{[passwordVariable]:password},
  serverEnvironment:{OPENCODE_SERVER_PASSWORD:password,OPENCODE_SERVER_USERNAME:'opencode'},
  serverArgs:['serve','--port',String(port),'--hostname','127.0.0.1','--pure'],
 };
}

export async function verifyOpenCodeAuthentication(baseUrl,password,request=fetch){
 const authorization=`Basic ${Buffer.from(`opencode:${password}`).toString('base64')}`;
 const authenticated=await request(`${baseUrl}provider`,{headers:{Authorization:authorization},signal:AbortSignal.timeout(1000)});
 if(!authenticated.ok)return false;
 const anonymous=await request(`${baseUrl}provider`,{signal:AbortSignal.timeout(1000)});
 return anonymous.status===401;
}
