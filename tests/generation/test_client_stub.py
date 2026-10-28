import importlib.util,pathlib,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('generator',ROOT/'scripts/generate_contracts.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
class ClientStubs(unittest.TestCase):
 def test_open_ended_rpc_results_remain_any_without_breaking_generation(self):
  schema={'$defs':{'MethodInputs':{'properties':{'diagnostics.report':{'type':'object'}}},'MethodOutputs':{'properties':{'diagnostics.report':True}}}}
  source=g.client_stub(schema);compile(source,'client.pyi','exec');self.assertIn('def report',source);self.assertIn('-> Any',source)
if __name__=='__main__':unittest.main()
