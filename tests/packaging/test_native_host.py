import importlib.util,json,pathlib,tempfile,unittest,hashlib
ROOT=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('native_host',ROOT/'scripts/native_host.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class NativeHost(unittest.TestCase):
 def test_generated_registration_is_explicit_and_has_exact_origin(self):
  with tempfile.TemporaryDirectory()as d:
   root=pathlib.Path(d);binary=root/'source.exe';binary.write_bytes(b'fixture-native-binary');out=root/'new-host';path=m.prepare(binary,out,'abcdefghijklmnopabcdefghijklmnop')
   manifest=json.loads(path.read_text());self.assertEqual(manifest['type'],'stdio');self.assertEqual(manifest['allowed_origins'],['chrome-extension://abcdefghijklmnopabcdefghijklmnop/']);self.assertTrue(pathlib.Path(manifest['path']).is_file());self.assertEqual(pathlib.Path(manifest['path']).read_bytes(),binary.read_bytes());self.assertTrue(pathlib.Path(manifest['path']).with_suffix('.json').is_file())
 def test_existing_installation_is_never_overwritten(self):
  with tempfile.TemporaryDirectory()as d:
   root=pathlib.Path(d);binary=root/'s';binary.write_bytes(b'x');out=root/'exists';out.mkdir();sentinel=out/'keep';sentinel.write_text('keep')
   with self.assertRaises(FileExistsError):m.prepare(binary,out,'a'*32)
   self.assertEqual(sentinel.read_text(),'keep')
 def test_invalid_extension_identifiers_fail_before_creating_files(self):
  with tempfile.TemporaryDirectory()as d:
   root=pathlib.Path(d);binary=root/'s';binary.write_bytes(b'x')
   for id in ['*','a'*31,'q'*32,'https://invalid','../escape']:
    out=root/'new'
    with self.assertRaises(ValueError):m.prepare(binary,out,id)
    self.assertFalse(out.exists())
 def test_install_receipt_checksums_all_owned_payloads(self):
  with tempfile.TemporaryDirectory()as d:
   root=pathlib.Path(d);binary=root/'s';binary.write_bytes(b'x');out=root/'new';m.prepare(binary,out,'b'*32,['--allow-passive-discovery'])
   receipt=json.loads((out/'receipt.json').read_text());self.assertEqual(len(receipt['files']),3)
   for name,digest in receipt['files'].items():self.assertEqual(hashlib.sha256((out/name).read_bytes()).hexdigest(),digest)
