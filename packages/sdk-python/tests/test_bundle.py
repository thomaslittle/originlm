import hashlib,json,pathlib,platform,sys,tempfile,unittest
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[1]/'src'))
from originlm.bundle import resolve_bundle_binary
from originlm.errors import LocalAIError
class Bundles(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.root=pathlib.Path(self.tmp.name);(self.root/'bin').mkdir();self.name='bin/originlm'+('.exe' if sys.platform=='win32' else '');self.bytes=b'synthetic-native-file';(self.root/self.name).write_bytes(self.bytes)
  cpu='aarch64' if platform.machine().lower() in ('arm64','aarch64') else 'x86_64';suffix={'win32':'pc-windows-msvc','darwin':'apple-darwin'}.get(sys.platform,'unknown-linux-gnu')
  self.manifest={'schemaVersion':1,'version':'0.1.0-alpha.1','platform':cpu+'-'+suffix,'files':{self.name:{'bytes':len(self.bytes),'sha256':hashlib.sha256(self.bytes).hexdigest()}}};self.save()
 def save(self):(self.root/'manifest.json').write_text(json.dumps(self.manifest),encoding='utf-8')
 def tearDown(self):self.tmp.cleanup()
 def test_exact_native_file_is_verified_without_spawning(self):self.assertEqual(resolve_bundle_binary(str(self.root)),str(self.root/self.name))
 def test_modified_bytes_and_wrong_platform_are_rejected(self):
  (self.root/self.name).write_bytes(b'changed')
  with self.assertRaises(LocalAIError):resolve_bundle_binary(str(self.root))
  (self.root/self.name).write_bytes(self.bytes);self.manifest['platform']='another-platform';self.save()
  with self.assertRaisesRegex(LocalAIError,'PLATFORM_UNSUPPORTED'):resolve_bundle_binary(str(self.root))
 def test_relative_path_and_missing_manifest_have_safe_errors(self):
  with self.assertRaises(LocalAIError):resolve_bundle_binary('relative')
  (self.root/'manifest.json').unlink()
  with self.assertRaises(LocalAIError) as error:resolve_bundle_binary(str(self.root))
  self.assertNotIn(str(self.root),str(error.exception))
 def test_symlink_is_not_followed(self):
  binary=self.root/self.name;binary.unlink()
  try:binary.symlink_to(self.root/'manifest.json')
  except OSError:self.skipTest('OS link permission unavailable')
  with self.assertRaises(LocalAIError):resolve_bundle_binary(str(self.root))
if __name__=='__main__':unittest.main()
