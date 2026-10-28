"""Opt-in end-to-end native metadata check. Never submits a generation prompt."""
import argparse,json,pathlib,sys,os
root=pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0,str(root/'packages/sdk-python/src'))
from originlm import Client
parser=argparse.ArgumentParser()
parser.add_argument('--provider',choices=['codex','claude_code'],required=True)
parser.add_argument('--allow-metadata',action='store_true',required=True)
parser.add_argument('--binary',default=str(root/'target/debug'/('originlm.exe' if os.name=='nt' else 'originlm')))
args=parser.parse_args()
with Client.from_stdio(executable=str(pathlib.Path(args.binary).resolve()),args=['serve','--stdio','--allow-passive-discovery','--allow-metadata',args.provider]) as client:
 client.discovery.refresh({'level':'passive'})
 before=client.snapshot.get();connection=next((c for c in before['connections'] if c['providerId']==args.provider and c['status']=='detected'),None)
 if connection is None:raise SystemExit('No native provider connection detected')
 result=client.connections.probe({'connectionId':connection['id']})
 snapshot=client.snapshot.get();connection=next(c for c in snapshot['connections'] if c['id']==connection['id'])
 routes={r['id'] for r in snapshot['routes'] if r['connectionId']==connection['id']}
 models=[m for m in snapshot['models'] if m['routeId'] in routes]
 pools=[p for p in snapshot['quotaPools'] if connection['id'] in p['connectionIds']]
 print(json.dumps({'provider':args.provider,'probe':result['status'],'auth':connection['auth'].get('value','unknown'),'planReported':connection['plan']['state']=='known','models':len(models),'nativeOptionDescriptors':sum(len(m['options'])for m in models),'usageWindows':sum(len(p['windows'])for p in pools),'generationRequests':0}))
