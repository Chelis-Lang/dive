#!/usr/bin/env python3
"""Corrupt portable delivery fixtures without changing retained producer evidence."""
import gzip
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('delivery', HERE / 'verify-delivery.py')
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class IntegrityControls(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='dive-definition-forms-corruption-')
        self.base = Path(self.temporary.name)
        for name in ('runtime-source', 'reports', 'gate-logs'):
            shutil.copytree(HERE / name, self.base / name)
        for name in ('delivery.json', 'runtime-records.json.gz', 'packaging-controls.json', 'packaging-controls.json.gz'):
            shutil.copy2(HERE / name, self.base / name)
        self.delivery = MODULE.read_json(self.base / 'delivery.json')

    def tearDown(self):
        self.temporary.cleanup()

    def save(self):
        (self.base / 'delivery.json').write_text(json.dumps(self.delivery))

    def replace_pack(self, rows):
        packed = gzip.compress(json.dumps(rows).encode(), mtime=0)
        (self.base / 'runtime-records.json.gz').write_bytes(packed)
        self.delivery['records_archive_sha256'] = MODULE.digest(packed)
        self.save()

    def test_baseline(self):
        self.assertEqual(MODULE.verify(self.base)['runtime_records'], 16611)

    def test_corrupt_raw_with_new_outer_hash(self):
        rows = json.loads(gzip.decompress((self.base / 'runtime-records.json.gz').read_bytes()))
        rows[0]['data'] = 'YWx0ZXJlZA=='
        self.replace_pack(rows)
        with self.assertRaisesRegex(ValueError, 'runtime bytes'):
            MODULE.verify(self.base)

    def test_missing_raw_with_new_outer_hash(self):
        rows = json.loads(gzip.decompress((self.base / 'runtime-records.json.gz').read_bytes()))
        self.replace_pack(rows[1:])
        with self.assertRaisesRegex(ValueError, 'missing runtime'):
            MODULE.verify(self.base)

    def test_wrong_source_hash_with_new_producer_hash(self):
        path = self.base / self.delivery['source_archive']
        producer = MODULE.read_json(path)
        producer['source']['Cargo.toml']['raw_sha256'] = '0' * 64
        path.write_text(json.dumps(producer))
        self.delivery['source_archive_sha256'] = MODULE.digest(path.read_bytes())
        self.save()
        with self.assertRaisesRegex(ValueError, 'source bytes'):
            MODULE.verify(self.base)

    def test_forged_proof_with_new_report_hash(self):
        path = self.base / 'reports/public.json'
        report = MODULE.read_json(path)
        report['model_proved'] = True
        path.write_text(json.dumps(report))
        self.delivery['reports_sha256']['public.json'] = MODULE.digest(path.read_bytes())
        self.save()
        with self.assertRaisesRegex(ValueError, 'public scope'):
            MODULE.verify(self.base)


if __name__ == '__main__':
    unittest.main()
