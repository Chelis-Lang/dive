#!/usr/bin/env python3
"""Negative controls: reference ambiguity cannot excuse invalid proof claims."""
import copy
import importlib.util
from pathlib import Path
import unittest

path = Path(__file__).with_name('compare.py')
spec = importlib.util.spec_from_file_location('dfid_comparison', path)
comparison = importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)


class ComparisonControls(unittest.TestCase):
    def test_ambiguity_keeps_stable_obligations(self):
        row = dict(model='DfidBranch', codes=[2110, 2121], exit=12,
                   completion_message=False, generated=7, distinct=5)
        valid = dict(result='violation', observed_tlc_code=2110,
                     violation_validated=True, model_proved=False, exhaustive=False,
                     statistics=dict(generated=123, distinct=99))
        self.assertEqual(comparison.stable_mismatches(row, valid, 12, True), [])
        for mutation in (dict(result='complete', model_proved=True, exhaustive=True),
                         dict(violation_validated=False), dict(result='internal-error')):
            bad = copy.deepcopy(valid)
            bad.update(mutation)
            self.assertTrue(comparison.stable_mismatches(row, bad, 12, True), mutation)
        self.assertIn('exit', comparison.stable_mismatches(row, valid, 0, True))
        self.assertIn('generated', comparison.stable_mismatches(row, valid, 12, False))

    def test_refusal_and_binding_error_cannot_hide_false_proof_metadata(self):
        for kind in ('unsupported', 'rejected', 'input-error'):
            valid = dict(result=kind, model_proved=False, exhaustive=False,
                         violation_validated=False, exit_code=3)
            self.assertEqual(comparison.metadata_mismatches(valid, 3), [])
            for mutation in (dict(model_proved=True), dict(exhaustive=True),
                             dict(violation_validated=True), dict(exit_code=0)):
                bad = dict(valid, **mutation)
                self.assertTrue(comparison.metadata_mismatches(bad, 3), mutation)

    def test_zero_exit_cannot_prove_the_infinite_counter(self):
        row = dict(model='DfidBound', codes=[], exit=0, completion_message=False,
                   generated=7, distinct=4)
        bad = dict(result='complete', observed_tlc_code=None, model_proved=True,
                   exhaustive=True, statistics=dict(generated=7, distinct=4))
        self.assertIn('unbounded-model-proof',
                      comparison.stable_mismatches(row, bad, 0, False))


if __name__ == '__main__':
    unittest.main()
