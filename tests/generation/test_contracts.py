import importlib.util, pathlib, unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('generator',ROOT/'scripts/generate_contracts.py')
generator=importlib.util.module_from_spec(spec);spec.loader.exec_module(generator)
SCHEMA={'title':'Contracts','$defs':{
 'Amount':{'type':'string'},
 'Record':{'type':'object','properties':{'value':{'$ref':'#/$defs/Amount'},'state':{'enum':['known','unknown']},'optional':{'type':['string','null']},'native-option':{'type':'array','items':{'type':'string'}}},'required':['value','state'],'additionalProperties':False},
 'Maybe':{'anyOf':[{'$ref':'#/$defs/Record'},{'type':'null'}]},
 'Map':{'type':'object','additionalProperties':{'$ref':'#/$defs/Record'}},
 'Future':{'oneOf':[{'type':'object','properties':{'kind':{'const':'future'},'choice':{'type':'string'}},'required':['kind','choice']},{'type':'object','properties':{'kind':{'const':'unknown'}}}]}
}}
class GeneratedTypes(unittest.TestCase):
 def test_typescript_preserves_required_optional_and_decimal_strings(self):
  source=generator.typescript(SCHEMA)
  self.assertIn('export type Amount = string;',source);self.assertIn('"value": Amount',source);self.assertIn('"optional"?:',source);self.assertIn('"native-option"?: Array<string>',source)
 def test_python_types_import_and_preserve_forward_references(self):
  source=generator.python_types(SCHEMA);namespace={};exec(compile(source,'generated.py','exec'),namespace)
  self.assertIn('Record',namespace);self.assertIn('value',namespace['Record'].__required_keys__);self.assertNotIn('optional',namespace['Record'].__required_keys__)
 def test_outputs_are_deterministic(self):
  self.assertEqual(generator.typescript(SCHEMA),generator.typescript(SCHEMA));self.assertEqual(generator.python_types(SCHEMA),generator.python_types(SCHEMA))
 def test_external_references_are_not_silently_accepted(self):
  bad={'$defs':{'Bad':{'$ref':'https://example.invalid/schema'}}}
  with self.assertRaises(ValueError):generator.typescript(bad)
  with self.assertRaises(ValueError):generator.python_types(bad)
 def test_unsafe_definition_names_fail_instead_of_generating_code(self):
  bad={'$defs':{'hello;process.exit()':{'type':'string'}}}
  with self.assertRaises(ValueError):generator.typescript(bad)
 def test_union_values_and_nested_shapes_are_not_erased(self):
  source=generator.typescript(SCHEMA);self.assertIn('Record | null',source);self.assertIn('"future"',source);self.assertIn('"choice": string',source)
if __name__=='__main__':unittest.main()
