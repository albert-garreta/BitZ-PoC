import ast, collections, copy, unittest
from pathlib import Path

source=ast.parse(Path(__file__).with_name('finalize.py').read_text())
function=next(n for n in source.body if isinstance(n,ast.FunctionDef) and n.name=='retain_consistent_integer_shapes')
namespace={'collections':collections}
exec(compile(ast.Module(body=[function],type_ignores=[]),'<report policy>','exec'),namespace)
retain=namespace[function.name]

def fixture(pattern, chosen, placement='cpu0'):
 return dict(family='integer_full_mac',size=pattern+'_l2_n33',placement=placement,
  chosen=chosen,baseline='circuit_z',decision='accept_candidate',baseline_median_ns=100.,
  chosen_median_ns=70.,chosen_median_ratio=.7,chosen_median_ci_high=.71,
  chosen_p95_ci_high=.72,chosen_worst_process=.73,baseline_allocation_calls=0,
  baseline_allocation_bytes=0,chosen_allocation_calls=0,chosen_allocation_bytes=0)

class ShapePolicy(unittest.TestCase):
 def test_both_fixtures_pass_same_candidate(self):
  rows=[fixture(p,'fused4') for p in ['full','carry']];before=copy.deepcopy(rows)
  retain(rows);self.assertEqual(rows,before)
 def test_one_fixture_failure_retains_baseline_for_both(self):
  rows=[fixture('full','fused4'),fixture('carry','circuit_z')]
  retain(rows);self.assertEqual([r['chosen'] for r in rows],['circuit_z']*2)
  self.assertEqual(rows[0]['chosen_median_ns'],100.)
  self.assertEqual(rows[0]['chosen_median_ratio'],1.)
 def test_cannot_dispatch_on_fixture_label(self):
  rows=[fixture('full','fused4'),fixture('carry','fused1')]
  retain(rows);self.assertEqual([r['chosen'] for r in rows],['circuit_z']*2)
 def test_cpu_placements_can_select_different_kernels(self):
  rows=[fixture(p,v,cpu) for cpu,v in [('cpu0','fused1'),('cpu8','fused4')] for p in ['full','carry']]
  before=copy.deepcopy(rows);retain(rows);self.assertEqual(rows,before)

unittest.main()
