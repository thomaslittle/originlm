import pathlib,sys,unittest
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[1]/'src'))
from originlm import AsyncClient
class Transport:
 def __init__(self,fail=False):self.calls=[];self.closes=0;self.fail=fail
 async def request(self,method,params,**options):
  self.calls.append(method)
  if method=='runtime.handshake':return {'protocolVersion':'1.0','methods':['runtime.closeClient']}
  if self.fail:raise RuntimeError('private native response')
  return {'closed':True}
 async def close(self):self.closes+=1
class Close(unittest.IsolatedAsyncioTestCase):
 async def test_closing_releases_native_owner_before_transport(self):
  t=Transport();c=await AsyncClient.connect(t);await c.close();await c.close();self.assertEqual(t.calls,['runtime.handshake','runtime.closeClient']);self.assertEqual(t.closes,1)
 async def test_failed_remote_cleanup_does_not_leak_or_prevent_shutdown(self):
  t=Transport(True);c=await AsyncClient.connect(t);await c.close();self.assertEqual(t.closes,1);self.assertEqual(len(t.calls),2)
