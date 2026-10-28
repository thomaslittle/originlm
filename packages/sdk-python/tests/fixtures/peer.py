"""Synthetic protocol peer: no credentials, files, subprocesses, or provider APIs."""
import json
import os
import sys
import threading
lock = threading.Lock()
def emit(value):
    with lock:
        sys.stdout.write(json.dumps(value, ensure_ascii=False) + '\n')
        sys.stdout.flush()
def result(identifier, value):
    emit({'jsonrpc': '2.0', 'id': identifier, 'result': value})
for line in sys.stdin:
    r = json.loads(line)
    method, p = r['method'], r.get('params', {})
    if method == 'runtime.handshake':
        result(r['id'], {'protocolVersion': '1.0', 'methods': ['snapshot.get', 'events.subscribe', 'events.unsubscribe']})
    elif method == 'test.hang':
        pass
    elif method == 'test.delay':
        threading.Timer(p['delay'], result, (r['id'], p['value'])).start()
    elif method == 'test.bad':
        sys.stdout.write('NOT JSON\n'); sys.stdout.flush()
    elif method == 'test.large':
        sys.stdout.write('x' * 2048); sys.stdout.flush()
    elif method == 'test.exit':
        sys.exit(0)
    elif method == 'test.error':
        emit({'jsonrpc':'2.0','id':r['id'],'error':{'code':-32000,'message':'SECRET_NATIVE_LOG','data':{'code':'AUTH_REQUIRED'}}})
    elif method == 'test.retry':
        emit({'jsonrpc':'2.0','id':r['id'],'error':{'code':-32000,'message':'SECRET_NATIVE_LOG','data':{'code':'RATE_LIMITED','retryable':True,'retryAfterMs':2750,'message':'PRIVATE'}}})
    elif method == 'test.gap':
        emit({'jsonrpc':'2.0','method':'events.error','params':{'subscriptionId':'synthetic','code':'EVENT_GAP'}})
        result(r['id'], {'ok':True})
    elif method == 'test.flood':
        for i in range(20):
            emit({'jsonrpc':'2.0','method':'events.event','params':{'subscriptionId':'synthetic','event':{'sequence':str(i+2)}}})
        result(r['id'], {'sent':True})
    elif method == 'events.subscribe':
        result(r['id'], {'subscriptionId':'synthetic'})
        for i in range(1 if p.get('after') == 'manual' else 3):
            emit({'jsonrpc':'2.0','method':'events.event','params':{'subscriptionId':'synthetic','event':{'sequence':str(i+1)}}})
    elif method == 'test.env':
        result(r['id'], {'injected':os.environ.get('PYTHONSTARTUP'), 'explicit':os.environ.get('EXPLICIT')})
    else:
        result(r['id'], p)
