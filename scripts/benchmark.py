"""Measure the actual local helper. No provider login, inference, or publication."""
import argparse,asyncio,json,math,os,pathlib,platform,statistics,subprocess,sys,tempfile,time
ROOT=pathlib.Path(__file__).resolve().parents[1]

def summarize(samples):
    values=list(samples)
    if not values or any(type(x) not in (int,float) or not math.isfinite(x) or x<0 for x in values):raise ValueError('Expected finite nonnegative measurements')
    ordered=sorted(values)
    return {'count':len(values),'min':ordered[0],'max':ordered[-1],'mean':statistics.mean(values),'p50':ordered[math.ceil(len(values)*0.50)-1],'p95':ordered[math.ceil(len(values)*0.95)-1]}

async def measure(bundle,idle_seconds):
    import psutil
    from release_support import verify_manifest,sha256
    manifest=json.loads((bundle/'manifest.json').read_text(encoding='utf-8'));verify_manifest(bundle,manifest)
    binary=bundle/'bin'/('originlm.exe' if os.name=='nt' else 'originlm')
    with tempfile.TemporaryDirectory(prefix='originlm-benchmark-') as temporary:
        home=pathlib.Path(temporary)
        for name in ['.codex',*['.codex-fixture-'+str(i) for i in range(9)]]:(home/name).mkdir()
        cold=[];count=0
        for _ in range(20):
            started=time.perf_counter()
            p=await asyncio.create_subprocess_exec(str(binary),'scan','--json','--home',str(home),stdout=asyncio.subprocess.PIPE,stderr=asyncio.subprocess.DEVNULL)
            raw,_=await asyncio.wait_for(p.communicate(),10)
            if p.returncode or len(raw)>1048576:raise RuntimeError('Cold inventory failed')
            snapshot=json.loads(raw);count=len(snapshot['connections']);cold.append((time.perf_counter()-started)*1000)
        p=await asyncio.create_subprocess_exec(str(binary),'serve','--stdio','--home',str(home),'--allow-passive-discovery',stdin=asyncio.subprocess.PIPE,stdout=asyncio.subprocess.PIPE,stderr=asyncio.subprocess.DEVNULL,limit=1048576)
        process=psutil.Process(p.pid);sequence=0
        async def request(method,params):
            nonlocal sequence
            sequence+=1;p.stdin.write((json.dumps({'jsonrpc':'2.0','id':sequence,'method':method,'params':params})+'\n').encode());await p.stdin.drain()
            raw=await asyncio.wait_for(p.stdout.readline(),10)
            if len(raw)>1048576:raise RuntimeError('Unexpected benchmark frame')
            result=json.loads(raw)
            if result.get('id')!=sequence or 'result' not in result:raise RuntimeError('Native benchmark request failed')
            return result['result']
        try:
            await request('runtime.handshake',{'protocolVersion':'1.0'});await request('discovery.refresh',{'level':'passive'})
            for _ in range(10):await request('snapshot.get',{})
            warm=[]
            for _ in range(200):
                started=time.perf_counter();await request('snapshot.get',{});warm.append((time.perf_counter()-started)*1000)
            times=process.cpu_times();cpu_start=times.user+times.system;started=time.perf_counter();rss=[]
            for _ in range(idle_seconds):rss.append(process.memory_info().rss);await asyncio.sleep(1)
            elapsed=time.perf_counter()-started;times=process.cpu_times();cpu_percent=(times.user+times.system-cpu_start)/elapsed*100
        finally:
            if p.returncode is None:
                p.terminate()
                try:await asyncio.wait_for(p.wait(),3)
                except TimeoutError:p.kill();await p.wait()
    sdk=json.loads(subprocess.check_output(['node','scripts/measure_sdk.mjs'],cwd=ROOT,text=True,timeout=60))
    report={'sourceCommit':manifest['sourceCommit'],'workingTreeDirty':manifest['workingTreeDirty'],'binarySha256':sha256(binary),'platform':manifest['platform'],'hostOS':platform.platform(),'logicalCPUs':os.cpu_count(),'fixture':{'detectedProfiles':count,'registeredPassiveAdapters':9,'providerCalls':0,'inferenceRequests':0},'coldProcessAndScanMs':summarize(cold),'warmStdioSnapshotMs':summarize(warm),'idle':{'seconds':elapsed,'oneCoreCPUPercent':cpu_percent,'peakRSSMiB':max(rss)/(1024*1024)},'browserSDK':sdk}
    report['targetsMet']={'coldScanP95Below1s':report['coldProcessAndScanMs']['p95']<1000,'warmIPCP95Below50ms':report['warmStdioSnapshotMs']['p95']<50,'idleBelow50MiB':report['idle']['peakRSSMiB']<50,'idleBelowOnePercent':cpu_percent<1,'browserSDKBelow25KiBGzip':sdk['gzipBytes']<25*1024}
    return report

def main():
    p=argparse.ArgumentParser();p.add_argument('--bundle',type=pathlib.Path);p.add_argument('--idle-seconds',type=int,default=60);p.add_argument('--enforce',action='store_true');args=p.parse_args()
    if not 5<=args.idle_seconds<=300:raise ValueError('Idle sampling must be between five and 300 seconds')
    bundle=args.bundle or ROOT/json.loads((ROOT/'dist/release/latest.json').read_text(encoding='utf-8'))['directory']
    report=asyncio.run(measure(bundle.resolve(),args.idle_seconds));out=ROOT/'.verification/performance.json';out.parent.mkdir(exist_ok=True);out.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8');print(json.dumps(report,indent=2))
    return 1 if args.enforce and not all(report['targetsMet'].values()) else 0
if __name__=='__main__':raise SystemExit(main())
