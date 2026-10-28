import asyncio, os, pathlib, sys, tempfile, unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'packages/sdk-python/src'))
from originlm import AsyncClient, AsyncStdioTransport, Client, LocalAIError
BINARY=os.environ.get('LAI_NATIVE_BINARY')
if not BINARY: raise RuntimeError('Run scripts/test_native_bridge.py with a compiled native helper')

def options(home,allow=True):
    return dict(executable=BINARY,args=['serve','--stdio',*(['--allow-passive-discovery'] if allow else []),'--home',home])

class NativeAsync(unittest.IsolatedAsyncioTestCase):
    async def test_native_discovery_and_revocation(self):
        with tempfile.TemporaryDirectory(prefix='originlm-python-') as home:
            pathlib.Path(home,'.codex').mkdir()
            secret=pathlib.Path(home,'.codex','auth.json');secret.write_text('PRIVATE-SENTINEL')
            async with await AsyncClient.connect(await AsyncStdioTransport.open(**options(home))) as ai:
                # Host-authorized inventory may already have populated this cache.
                initial=await ai.snapshot.get()
                self.assertEqual(initial['mode'],'live')
                self.assertTrue(all(c['auth']['state']=='unknown' for c in initial['connections']))
                self.assertEqual(initial['models'],[])
                self.assertEqual(initial['accounts'],[])
                self.assertNotIn('PRIVATE-SENTINEL',str(initial))
                await ai.discovery.refresh({'level':'passive'})
                snapshot=await ai.snapshot.get()
                self.assertEqual(snapshot['mode'],'live')
                self.assertTrue(any(c['providerId']=='codex' for c in snapshot['connections']))
                self.assertTrue(all(c['auth']['state']=='unknown' for c in snapshot['connections']))
                self.assertEqual(snapshot['models'],[])
                self.assertNotIn('PRIVATE-SENTINEL',str(snapshot))
                self.assertEqual(secret.read_text(),'PRIVATE-SENTINEL')
                values=await asyncio.gather(*(ai.snapshot.get({'fields':['mode']}) for _ in range(24)))
                self.assertTrue(all(v=={'mode':'live'} for v in values))
                await ai.grants.revoke()
                self.assertEqual((await ai.snapshot.get())['connections'],[])
                with self.assertRaises(LocalAIError) as caught: await ai.discovery.refresh()
                self.assertEqual(caught.exception.code,'LOCAL_ACCESS_DENIED')
    async def test_default_policy_denies_discovery(self):
        with tempfile.TemporaryDirectory(prefix='originlm-denied-') as home:
            async with await AsyncClient.connect(await AsyncStdioTransport.open(**options(home,False))) as ai:
                with self.assertRaises(LocalAIError) as caught: await ai.discovery.refresh()
                self.assertEqual(caught.exception.code,'LOCAL_ACCESS_DENIED')

class NativeSync(unittest.TestCase):
    def test_sync_facade_scans_empty_home_through_rust(self):
        with tempfile.TemporaryDirectory(prefix='originlm-sync-') as home:
            with Client.from_stdio(**options(home)) as ai:
                ai.discovery.refresh()
                self.assertEqual(ai.snapshot.get()['connections'],[])

if __name__=='__main__':unittest.main()
