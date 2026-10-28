import sys,json,os
mode=sys.argv[1]
if mode=='managed':
 log=os.environ.get('TEST_METHODS_LOG')
 if log:
  token_kind = 'initial' if os.environ.get('ACCESS_TOKEN') == 'token-old' else 'rotated' if os.environ.get('ACCESS_TOKEN') == 'token-new' else 'unknown'
  with open(log,'a',newline='\n') as f:f.write('process '+token_kind+'\n')
model={"id":"native-id","model":"test-native","displayName":"Test native","description":"Fixture","hidden":False,"isDefault":True,"supportedReasoningEfforts":[{"reasoningEffort":"future","description":"Future"}],"defaultReasoningEffort":"future","inputModalities":["text"]}
def send(v): print(json.dumps(v),flush=True)
def emit(method,params): send({"method":method,"params":params})
def reply(id,value): send({"id":id,"result":value})
turn=''
account_reads=0
for line in sys.stdin:
 r=json.loads(line);method=r.get('method');p=r.get('params',{});id=r.get('id')
 if mode=='managed' and log:
  with open(log,'a',newline='\n') as f:f.write(method+'\n')
 if method=='initialize':reply(id,{"userAgent":"codex/0.157.0" if mode=='managed' else "codex/0.156.1"})
 elif method=='initialized':pass
 elif method=='account/read':
  account_reads+=1
  if mode=='managed':reply(id,{"account":None,"requiresOpenaiAuth":False})
  else:reply(id,{"account":{"type":"chatgpt","email":"changed@example.test" if mode=='account-changed-before-turn' and account_reads>1 else "test@example.test","planType":"test"},"requiresOpenaiAuth":True})
 elif method=='model/list':reply(id,{"data":[model],"nextCursor":None})
 elif method=='account/rateLimits/read':reply(id,{"rateLimits":{}})
 elif method=='mcpServerStatus/list':
  assert mode.startswith('host')
  reply(id,{"data":[{"name":"design" if mode=='host_collision' else "user_global"}],"nextCursor":None})
 elif method=='thread/resume':
  assert mode.startswith('resume') or (mode=='managed' and os.environ.get('ACCESS_TOKEN')=='token-new'), mode
  assert p['threadId']=='native-thread' and p['excludeTurns'] is True and p['sandbox']=='read-only'
  assert p['config']['features.shell_tool'] is False and 'ephemeral' not in p
  if mode=='resume_missing':send({"id":id,"error":{"code":-32600,"message":"thread not found: native-thread"}});continue
  reply(id,{"thread":{"id":"other-thread" if mode=='resume_other' else "native-thread"},"model":"test-native","sandbox":{"type":"readOnly"},"approvalPolicy":"never","cwd":os.getcwd()})
 elif method=='thread/start':
  assert not mode.startswith('resume') and not (mode=='managed' and os.environ.get('ACCESS_TOKEN')=='token-new'), 'resume must not start a new thread'
  assert p['ephemeral'] is (mode not in ('persist','managed')) and p['sandbox']=='read-only' and p['serviceTier']=='default'
  assert p['config']['features.shell_tool'] is False and p['config']['features.unified_exec'] is False
  if mode.startswith('host'):
   expected={'url':'http://127.0.0.1:48123/mcp','http_headers':{'Authorization':'Bearer '+'k'*40},'enabled_tools':['echo_color'],'default_tools_approval_mode':'approve','required':True}
   assert p['config']['mcp_servers.design']==expected, p['config']
   assert p['config']['features.code_mode.direct_only_tool_namespaces']==['mcp__design'], p['config']
   assert p['config']['features.non_prefixed_mcp_tool_names'] is False, p['config']
   assert p['approvalPolicy']=='never'
   assert p['config']['mcp_servers.user_global.enabled'] is False, p['config']
  reply(id,{"thread":{"id":"native-thread"},"model":"test-native","sandbox":{"type":"dangerFullAccess" if mode=='unsafe' else 'readOnly'},"approvalPolicy":"onRequest","cwd":os.getcwd()})
 elif method=='turn/start':
  assert p['model']=='test-native' and p['effort']=='future'
  assert p['sandboxPolicy']=={'type':'readOnly'}
  if mode=='image':assert p['input']==[{'type':'text','text':'describe'},{'type':'image','url':'data:image/png;base64,AQID'},{'type':'image','url':'data:image/png;base64,AQID'}], p['input']
  else:assert p['input']==[{'type':'text','text':p['input'][0]['text']}], p['input']
  turn='turn-'+str(id)
  if mode=='early':emit('item/agentMessage/delta',{"threadId":"native-thread","turnId":turn,"delta":"early "})
  reply(id,{"turn":{"id":turn,"status":"inProgress"}})
  if mode=='approval':send({"id":900,"method":"item/commandExecution/requestApproval","params":{"threadId":"native-thread","turnId":turn,"command":"echo fixture","cwd":os.getcwd()}})
  elif mode=='hang':pass
  elif mode.startswith('host'):
   kind,server,tool={'host':('mcpToolCall','design','echo_color'),'host_other_server':('mcpToolCall','user_global','echo_color'),'host_command':('commandExecution','',''),'host_web':('webSearch','','')}[mode]
   item={'type':kind,'id':'item-1','server':server,'tool':tool,'arguments':{},'status':'inProgress'}
   emit('item/started',{"threadId":"native-thread","turnId":turn,"item":item})
   emit('item/completed',{"threadId":"native-thread","turnId":turn,"item":{**item,'status':'completed'}})
   emit('item/agentMessage/delta',{"threadId":"native-thread","turnId":turn,"delta":"color is red"})
   emit('turn/completed',{"threadId":"native-thread","turn":{"id":turn,"status":"completed"}})
  elif mode=='account-switch':emit('account/updated',{"authMode":None})
  elif mode=='token-refresh':
   emit('account/updated',{"authMode":"chatgpt","planType":"refreshed"})
   emit('item/agentMessage/delta',{"threadId":"native-thread","turnId":turn,"delta":"native reply"})
   emit('turn/completed',{"threadId":"native-thread","turn":{"id":turn,"status":"completed"}})
  else:
   emit('item/agentMessage/delta',{"threadId":"native-thread","turnId":turn,"delta":"native reply"})
   emit('turn/completed',{"threadId":"native-thread","turn":{"id":turn,"status":"completed"}})
 elif method=='turn/interrupt':reply(id,{});emit('turn/completed',{"threadId":"native-thread","turn":{"id":turn,"status":"interrupted"}})
 elif id==900:
  assert r['result']['decision']=='decline'
  emit('turn/completed',{"threadId":"native-thread","turn":{"id":turn,"status":"completed"}})
 else:send({"id":id,"error":{"code":-32601,"message":"Unsupported fixture operation"}})
