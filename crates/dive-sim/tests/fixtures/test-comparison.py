#!/usr/bin/env python3
"""Negative controls for sampling proof, process and witness evidence."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('comparison', Path(__file__).with_name('compare.py'))
comparison = importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)


class ComparisonControls(unittest.TestCase):
    def setUp(self):
        self.report = dict(command='simulate', profile='tlc', result='violation',
                           exhaustive=False, model_proved=False, exit_code=12,
                           violation_validated=True, tlc_code=2110, message=None,
                           statistics=dict(generated=3),
                           traces=[dict(tlc_states=[dict(x=0), dict(x=2)],
                                        validation=dict(result='valid'))])
        self.row = dict(native_scope='supported', exit=12, codes=[2110], generated=3, states=[0, 2])

    def test_genuine_violation(self):
        self.assertEqual(comparison.error_comparison(self.row, self.report, 12), ('agree', []))

    def test_supported_rows_cannot_hide_proof_exit_or_validation_forgeries(self):
        for mutation in [dict(exhaustive=True), dict(model_proved=True), dict(exit_code=99),
                         dict(violation_validated=False), dict(result='internal-error'),
                         dict(result='sampled'), dict(profile='dive')]:
            with self.subTest(mutation=mutation):
                report = copy.deepcopy(self.report)
                report.update(mutation)
                self.assertEqual(comparison.error_comparison(self.row, report, 12)[0], 'disagree')

    def test_refusal_is_not_an_evidence_exemption(self):
        row = dict(self.row, native_scope='unsupported')
        report = dict(self.report, result='unsupported', exit_code=3,
                      violation_validated=False, message='named unsupported feature', traces=[])
        self.assertEqual(comparison.error_comparison(row, report, 3), ('unsupported', []))
        for mutation in [dict(exhaustive=True), dict(model_proved=True), dict(exit_code=99),
                         dict(violation_validated=True), dict(message='')]:
            with self.subTest(mutation=mutation):
                forged = dict(report, **mutation)
                self.assertEqual(comparison.error_comparison(row, forged, 3)[0], 'disagree')

    def test_a_former_refusal_now_supported_must_agree(self):
        row = dict(self.row, native_scope='unsupported')
        self.assertEqual(comparison.error_comparison(row, self.report, 12), ('agree', []))
        for mutation in [dict(violation_validated=False), dict(tlc_code=2110 + 1),
                         dict(result='sampled')]:
            with self.subTest(mutation=mutation):
                forged = dict(self.report, **mutation)
                self.assertEqual(comparison.error_comparison(row, forged, 12)[0], 'disagree')

    def test_corrupt_ending_or_uncertified_witness_cannot_agree(self):
        for field, value in [('tlc_states', [dict(x=0), dict(x=7)]),
                             ('validation', dict(result='unsupported'))]:
            with self.subTest(field=field):
                report = copy.deepcopy(self.report)
                report['traces'][0][field] = value
                self.assertEqual(comparison.error_comparison(self.row, report, 12)[0], 'disagree')

    def test_sample_is_never_exhaustive(self):
        row = dict(exit=0, generated=3, traces=[[0, 2]])
        report = dict(self.report, result='sampled', exit_code=0, violation_validated=False, tlc_code=None)
        self.assertEqual(comparison.selection_mismatches(row, report, 0)[0], [])
        for mutation in [dict(exhaustive=True), dict(model_proved=True), dict(exit_code=99),
                         dict(violation_validated=True)]:
            self.assertTrue(comparison.selection_mismatches(row, dict(report, **mutation), 0)[0])


if __name__ == '__main__':
    unittest.main()
