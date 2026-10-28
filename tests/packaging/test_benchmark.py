import importlib.util,pathlib,unittest
ROOT=pathlib.Path(__file__).resolve().parents[2];s=importlib.util.spec_from_file_location('benchmark',ROOT/'scripts/benchmark.py');m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
class Measurements(unittest.TestCase):
 def test_nearest_rank_quantiles_are_measured_not_target_values(self):
  x=m.summarize(list(range(1,101)));self.assertEqual(x['p50'],50);self.assertEqual(x['p95'],95);self.assertEqual(x['mean'],50.5);self.assertEqual(x['count'],100)
 def test_empty_negative_and_nonfinite_samples_are_rejected(self):
  for samples in ([],[-1],[float('nan')],[float('inf')],[True],['2']):
   with self.subTest(samples=samples),self.assertRaises(ValueError):m.summarize(samples)
 def test_single_measurement_has_exact_quantiles(self):self.assertEqual(m.summarize([0.25])['p95'],0.25)
if __name__=='__main__':unittest.main()
