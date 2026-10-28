"""Synthetic process-tree fixture; a 30-second watchdog bounds leaked fixtures."""
import sys,os,time,pathlib,subprocess,json
if sys.argv[1]=='heartbeat':
 marker=pathlib.Path(sys.argv[2]);end=time.monotonic()+30
 while time.monotonic()<end:
  with marker.open('ab') as file:file.write(b'x')
  time.sleep(.03)
 sys.exit(0)
subprocess.Popen([sys.executable,__file__,'heartbeat',sys.argv[2]],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
deadline=time.monotonic()+1
while not pathlib.Path(sys.argv[2]).exists() and time.monotonic()<deadline:time.sleep(.01)
for line in sys.stdin:
 r=json.loads(line)
 if r.get('method')=='initialize':print(json.dumps({'id':r['id'],'result':{'userAgent':'codex/fixture'}}),flush=True)
 elif r.get('method')=='account/read':print(json.dumps({'id':r['id'],'result':{'account':None,'requiresOpenaiAuth':True}}),flush=True)
