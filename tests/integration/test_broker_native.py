import asyncio,json,os,pathlib,subprocess,tempfile,unittest,sys,uuid
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path.insert(0,str(ROOT/'packages/sdk-python/src'))
from originlm import AsyncClient,AsyncStdioTransport
BINARY=os.environ.get('LAI_NATIVE_BINARY',str(ROOT/'target/debug'/('originlm.exe' if os.name=='nt' else 'originlm')))
class BrokerTest(unittest.IsolatedAsyncioTestCase):
 async def test_python_client_uses_the_shared_native_broker_without_provider_logic(self):
  with tempfile.TemporaryDirectory()as home:
   address='originlm-'+uuid.uuid4().hex if os.name=='nt' else str(pathlib.Path(home)/'broker.sock')
   p=subprocess.Popen([BINARY,'serve','--ipc',address,'--home',home,'--allow-passive-discovery'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
   try:
    data=json.loads(await asyncio.wait_for(asyncio.to_thread(p.stdout.readline),5))
    self.assertTrue(hasattr(AsyncStdioTransport,'open_broker'))
    transport=await AsyncStdioTransport.open_broker(executable=BINARY,address=address,token=data['token'])
    async with await AsyncClient.connect(transport) as ai:
     await ai.discovery.refresh();self.assertEqual((await ai.snapshot.get())['mode'],'live')
   finally:
    p.stdin.close();await asyncio.to_thread(p.wait,5);p.stdout.close()
if __name__=='__main__':unittest.main()
