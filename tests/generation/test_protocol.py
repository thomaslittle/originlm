import importlib.util,pathlib,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('generator',ROOT/'scripts/generate_contracts.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
S={'$defs':{'Snapshot':{'type':'object','properties':{'revision':{'type':'string'}}},'EmptyParams':{'type':'object','additionalProperties':False},'MethodInputs':{'properties':{'snapshot.get':{'$ref':'#/$defs/EmptyParams'}}},'MethodOutputs':{'properties':{'snapshot.get':{'$ref':'#/$defs/Snapshot'}}}}}
class ProtocolDocuments(unittest.TestCase):
 def test_manifest_preserves_method_names_and_schema_refs(self):
  manifest,api=g.protocol_documents(S);self.assertEqual(manifest['protocolVersion'],'1.0');self.assertEqual(manifest['methods'][0]['name'],'snapshot.get');self.assertEqual(manifest['methods'][0]['resultSchema']['$ref'],'#/$defs/Snapshot')
 def test_api_documents_actual_http_routes_and_authenticated_rpc(self):
  _,api=g.protocol_documents(S);self.assertEqual(api['openapi'],'3.1.0');self.assertIn('/v1/rpc',api['paths']);self.assertIn('/v1/snapshot',api['paths']);self.assertEqual(api['security'],[{'localBearer':[]}]);self.assertEqual(api['paths']['/v1/pair']['post']['security'],[])
 def test_component_refs_are_rewritten_without_remote_fetches(self):
  _,api=g.protocol_documents(S);text=str(api);self.assertNotIn('#/$defs/',text);self.assertIn('#/components/schemas/Snapshot',text)
 def test_manifest_rejects_mismatched_method_maps(self):
  with self.assertRaises(ValueError):g.protocol_documents({'$defs':{'MethodInputs':{'properties':{'a':{}}},'MethodOutputs':{'properties':{}}}})
if __name__=='__main__':unittest.main()
