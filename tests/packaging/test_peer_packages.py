import importlib.util,json,pathlib,tempfile,unittest,sys
ROOT=pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
spec=importlib.util.spec_from_file_location('smoke',ROOT/'scripts/package_smoke.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class Peers(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory();self.root=pathlib.Path(self.temp.name)
 def tearDown(self):self.temp.cleanup()
 def package(self,name,deps=None):
  p=self.root/'node_modules'/name;p.mkdir(parents=True,exist_ok=True);(p/'package.json').write_text(json.dumps({'name':name,'version':'1.0.0','dependencies':deps or {}}),encoding='utf-8')
 def test_offline_peers_include_their_actual_installed_dependencies(self):
  self.package('react');self.package('react-dom',{'scheduler':'^1.0.0'});self.package('scheduler')
  self.assertEqual({p.name for p in m.peer_packages(self.root)},{'react','react-dom','scheduler'})
 def test_missing_peer_is_an_error_not_a_forced_install(self):
  self.package('react')
  with self.assertRaises(ValueError):m.peer_packages(self.root)
 def test_dependency_names_cannot_escape_the_installed_tree(self):
  self.package('react');self.package('react-dom',{'../outside':'*'})
  with self.assertRaises(ValueError):m.peer_packages(self.root)
 def test_cycles_do_not_repeat_package_work(self):
  self.package('react',{'react-dom':'*'});self.package('react-dom',{'react':'*'})
  self.assertEqual(len(m.peer_packages(self.root)),2)
if __name__=='__main__':unittest.main()
