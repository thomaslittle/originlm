import importlib.util,json,pathlib,tempfile,unittest,zipfile
ROOT=pathlib.Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('release_support',ROOT/'scripts/release_support.py');r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
class ReleaseBundles(unittest.TestCase):
 def setUp(self):self.tmp=tempfile.TemporaryDirectory();self.root=pathlib.Path(self.tmp.name)/'bundle';self.root.mkdir();(self.root/'bin').mkdir();(self.root/'bin/helper').write_bytes(b'owned-native-binary');(self.root/'README.md').write_text('installation',encoding='utf-8')
 def tearDown(self):self.tmp.cleanup()
 def manifest(self):return r.make_manifest(self.root,{'version':'0.1.0-alpha.1','platform':'test','sourceCommit':'a'*40})
 def test_manifest_has_exact_bytes_hashes_and_no_private_paths(self):
  m=self.manifest();self.assertEqual(m['files']['bin/helper']['bytes'],19);self.assertEqual(len(m['files']['bin/helper']['sha256']),64);self.assertNotIn(str(self.root),json.dumps(m));r.verify_manifest(self.root,m)
 def test_changed_missing_or_unlisted_files_are_not_valid(self):
  m=self.manifest();(self.root/'bin/helper').write_bytes(b'changed');
  with self.assertRaises(ValueError):r.verify_manifest(self.root,m)
  (self.root/'bin/helper').unlink()
  with self.assertRaises(ValueError):r.verify_manifest(self.root,m)
  (self.root/'bin/helper').write_bytes(b'owned-native-binary');(self.root/'unexpected').write_text('extra')
  with self.assertRaises(ValueError):r.verify_manifest(self.root,m)
 def test_traversal_absolute_and_duplicate_separator_paths_are_rejected(self):
  for key in ['../escape','/absolute','C:/secret','bin/../helper','bin\\helper','bin//helper']:
   m=self.manifest();m['files'][key]={'bytes':0,'sha256':'0'*64}
   with self.subTest(key=key),self.assertRaises(ValueError):r.verify_manifest(self.root,m)
 def test_archive_is_deterministic_and_records_no_absolute_paths(self):
  a=pathlib.Path(self.tmp.name)/'one.zip';b=pathlib.Path(self.tmp.name)/'two.zip';r.create_archive(self.root,a);r.create_archive(self.root,b);self.assertEqual(a.read_bytes(),b.read_bytes())
  with zipfile.ZipFile(a)as z:self.assertEqual(sorted(z.namelist()),['README.md','bin/helper']);self.assertTrue(all(not n.startswith('/') for n in z.namelist()))
 def test_manifest_file_is_not_self_hashed(self):
  m=self.manifest();(self.root/'manifest.json').write_text(json.dumps(m));self.assertNotIn('manifest.json',self.manifest()['files']);r.verify_manifest(self.root,m)
 def test_linked_files_are_never_followed(self):
  link=self.root/'linked'
  try:link.symlink_to(self.root/'README.md')
  except OSError:self.skipTest('OS symlink creation permission unavailable')
  with self.assertRaises(ValueError):self.manifest()
if __name__=='__main__':unittest.main()
