import asyncio, pathlib, sys, unittest
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[1]/'src'))
from originlm import AsyncStdioTransport,LocalAIError
class EventsError(unittest.IsolatedAsyncioTestCase):
 async def test_gap_does_not_kill_unrelated_requests(self):
  transport=await AsyncStdioTransport.open(executable=sys.executable,args=[str(pathlib.Path(__file__).parent/'fixtures/peer.py')])
  try:
   iterator=transport.subscribe({'after':'manual'});await anext(iterator)
   self.assertEqual(await transport.request('test.gap'),{'ok':True})
   with self.assertRaisesRegex(LocalAIError,'EVENT_GAP'):await anext(iterator)
   self.assertEqual(await transport.request('test.echo',{'alive':True}),{'alive':True})
  finally:await transport.close()
