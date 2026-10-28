import sys,json
mode=sys.argv[1]
# Persisted sessions name (--session-id) or reattach (--resume) the native id.
SID='fixture-session'
for flag in ('--session-id','--resume'):
 if flag in sys.argv:SID=sys.argv[sys.argv.index(flag)+1]
if 'persist' in mode:
 assert '--no-session-persistence' not in sys.argv
 assert ('--resume' in sys.argv)==(mode=='persist_resume')
 assert ('--session-id' in sys.argv)==(mode=='persist')
def send(value):print(json.dumps(value),flush=True)
def reply(r,result):send({'type':'control_response','response':{'subtype':'success','request_id':r['request_id'],'response':result}})
for line in sys.stdin:
 r=json.loads(line)
 if r.get('type')=='control_request':
  subtype=r['request']['subtype']
  if subtype=='initialize':reply(r,{'account':{'email':'person@example.test','subscriptionType':'Claude Test Plan','apiProvider':'firstParty'},'models':[{'value':'native','displayName':'Native','supportsEffort':True,'supportedEffortLevels':['future-effort']}],'current_permission_mode':'dontAsk'})
  elif subtype=='get_usage':reply(r,{'rate_limits_available':False})
  elif subtype=='interrupt':reply(r,{})
  elif subtype=='mcp_status':
   status={'mcp_failed':'failed'}.get(mode,'connected')
   reply(r,{'mcpServers':[{'name':'design','status':status,'tools':[{'name':'echo_color'}]}]})
  else:send({'type':'control_response','response':{'subtype':'error','request_id':r['request_id'],'error':'unsupported'}})
 elif r.get('type')=='user':
  assert r['client_composed'] is True
  if mode=='hang':continue
  if mode=='image':
   c=r['message']['content']
   assert isinstance(c,list) and len(c)==3, c
   for b in c[:2]:
    assert b['type']=='image' and b['source']['type']=='base64' and b['source']['media_type']=='image/png' and b['source']['data']=='AQID', b
   assert c[2]=={'type':'text','text':'describe'}, c[2]
  else:assert isinstance(r['message']['content'],str)
  if mode=='tool':send({'type':'control_request','request_id':'native-permission','request':{'subtype':'can_use_tool','tool_name':'Bash','input':{'command':'echo prohibited'}}});continue
  if mode=='host_tool':
   send({'type':'system','subtype':'init','session_id':SID,'tools':['mcp__design__echo_color']})
   send({'type':'stream_event','session_id':SID,'event':{'type':'content_block_start','index':0,'content_block':{'type':'tool_use','id':'tu-1','name':'mcp__design__echo_color','input':{}}}})
   send({'type':'assistant','session_id':SID,'message':{'content':[{'type':'tool_use','id':'tu-1','name':'mcp__design__echo_color','input':{'color':'red'}}]}})
   send({'type':'user','session_id':SID,'message':{'role':'user','content':[{'type':'tool_result','tool_use_id':'tu-1','content':'red'}]}})
   send({'type':'stream_event','session_id':SID,'event':{'type':'content_block_delta','delta':{'type':'text_delta','text':'color is red'}}})
   send({'type':'result','session_id':SID,'subtype':'success','is_error':False,'usage':{'input_tokens':10,'output_tokens':3}})
   continue
  if mode=='native_tool_use':
   send({'type':'system','subtype':'init','session_id':SID,'tools':['mcp__design__echo_color']})
   send({'type':'assistant','session_id':SID,'message':{'content':[{'type':'tool_use','id':'tu-2','name':'Read','input':{'file_path':'secret'}}]}})
   send({'type':'result','session_id':SID,'subtype':'success','is_error':False})
   continue
  if mode=='advertise_native':
   send({'type':'system','subtype':'init','session_id':SID,'tools':['mcp__design__echo_color','Bash']})
   send({'type':'result','session_id':SID,'subtype':'success','is_error':False})
   continue
  send({'type':'system','subtype':'init','session_id':SID,'tools':[]})
  send({'type':'stream_event','session_id':SID,'event':{'type':'content_block_delta','delta':{'type':'text_delta','text':'native Claude reply'}}})
  send({'type':'assistant','session_id':SID,'message':{'content':[{'type':'text','text':'native Claude reply'}]}})
  send({'type':'result','session_id':SID,'subtype':'success','is_error':False,'usage':{'input_tokens':10,'output_tokens':3}})
 elif r.get('type')=='control_response':
  assert r['response']['response']['behavior']=='deny'
  send({'type':'result','session_id':SID,'subtype':'error_during_execution','is_error':True})
