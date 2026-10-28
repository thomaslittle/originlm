"""Run real adapter suites and emit operation-level evidence, never inferred live support."""
import argparse,json,pathlib,subprocess,sys,platform
ROOT=pathlib.Path(__file__).resolve().parents[1]
SUITES={
 'codex':['codex_metadata','codex_process','codex_launch','execution'],
 'claude_code':['claude_metadata','claude_process'],
 'grok':['grok','acp_catalog','acp_process'],
 'google_acp_experimental':['acp_auth','native_profiles','native_launch'],
 'cursor':['cursor_metadata'],
 'http':['http','endpoint_config','http_catalogs','http_execution','http_frames'],
 'opencode':['opencode_catalog','opencode_execution'],
 'muse':['muse_catalog','muse_process'],
}
def main():
 p=argparse.ArgumentParser();p.add_argument('--all',action='store_true');p.add_argument('--adapter',choices=sorted(SUITES));args=p.parse_args()
 if not args.all and not args.adapter:p.error('Choose --all or --adapter')
 existing={p.stem for p in (ROOT/'crates/originlm-adapters/tests').glob('*.rs')};report=[]
 for name in sorted(SUITES) if args.all else [args.adapter]:
  suites=SUITES[name]
  missing=set(suites)-existing
  if missing:raise RuntimeError('Adapter conformance tests missing: '+','.join(sorted(missing)))
  command=['cargo','test','--locked','-p','originlm-adapters']
  for suite in suites:command+=['--test',suite]
  result=subprocess.run(command,cwd=ROOT,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=240)
  log=ROOT/'.verification'/('conformance-'+name+'.log');log.parent.mkdir(exist_ok=True);log.write_text(result.stdout+result.stderr,encoding='utf-8')
  print(name+': '+('PASS' if result.returncode==0 else 'FAIL'))
  report.append({'adapter':name,'testSuites':suites,'exitCode':result.returncode,'executedPlatform':platform.system(),'liveInferenceQualified':False})
  if result.returncode:print((result.stdout+result.stderr)[-5000:]);raise SystemExit(result.returncode)
 (ROOT/'.verification/adapter-conformance.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
if __name__=='__main__':
 try:main()
 except (OSError,ValueError,RuntimeError,subprocess.SubprocessError)as e:print(str(e),file=sys.stderr);raise SystemExit(1)
