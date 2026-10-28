"""Synthetic Codex protocol peer. Never contacts a provider or reads a real account."""
import json,sys,time,os,pathlib
mode=sys.argv[1]
log=pathlib.Path(sys.argv[2])
initialized=False
for raw in sys.stdin:
    item=json.loads(raw);method=item.get('method')
    with log.open('a',encoding='utf-8',newline='\n') as f:f.write(method+'\n')
    if method=='initialized':initialized=True;continue
    if method=='initialize':
        version='0.157.0' if mode=='managed' else '0.156.1'
        result={'userAgent':'codex_cli_rs/'+version,'codexHome':'PRIVATE-HOME','platformFamily':'synthetic','platformOs':'synthetic'}
    elif not initialized:raise RuntimeError('Initialization order violated')
    elif method=='account/read':
        assert item['params'].get('refreshToken') is False
        if mode=='slow':time.sleep(20)
        if mode=='malformed':print('{not json}',flush=True);continue
        if mode=='huge':print('x'*1100000,flush=True);continue
        reads=globals().get('reads',0)+1;globals()['reads']=reads
        email='other@example.test' if mode=='changed-account' and reads>1 else 'synthetic@example.test'
        plan='refreshed-plan' if mode=='token-refresh' and reads>1 else 'future-plan'
        result={'account':None if mode in ('signed-out','managed','managed-old') else {'type':'chatgpt','email':email,'planType':plan},'requiresOpenaiAuth':mode not in ('managed','managed-old')}
    elif method=='model/list':
        cursor=item['params'].get('cursor')
        number='2' if cursor else '1'
        model={'model':'synthetic-'+number,'displayName':'Synthetic '+number,'hidden':False,'isDefault':number=='1','supportedReasoningEfforts':[]}
        if mode=='service-tier' and cursor is None:
            model['serviceTiers']=[{'id':'priority','name':'Priority','description':''},{'id':'flex','name':'Flex','description':''}]
            model['defaultServiceTier']='flex'
        result={'data':[model],'nextCursor':('same-cursor' if mode=='loop' else ('next' if cursor is None else None))}
    elif method=='config/read':
        result={'config':({'service_tier':'fast'} if mode=='service-tier' else {})}
    elif method=='account/rateLimits/read':
        if mode=='usage-error':
            print(json.dumps({'id':item['id'],'error':{'code':-1,'message':'PRIVATE-TOKEN'}}),flush=True);continue
        result={'rateLimits':{'primary':{'usedPercent':12,'windowDurationMins':300}},'ordinaryUsageAllowed':True}
    else:raise RuntimeError('Forbidden method '+str(method))
    if mode=='server-request' and method=='account/read':
        print(json.dumps({'id':'approval-1','method':'item/commandExecution/requestApproval','params':{'command':'must never execute'}}),flush=True)
    if (mode=='initial-account' and method=='account/read') or (mode in ('changed-account','token-refresh') and method=='model/list'):
        print(json.dumps({'method':'account/updated','params':{'authMode':'chatgpt','planType':'future-plan'}}),flush=True)
    print(json.dumps({'method':'synthetic/notice','params':{'secret':'DO-NOT-EXPORT'}}),flush=True)
    print(json.dumps({'id':item['id'],'result':result}),flush=True)
