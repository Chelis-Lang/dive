#!/usr/bin/env python3
"""Verify the complete immutable disjoint-procedure campaign and failed epochs."""
import argparse, ast, base64, gzip, hashlib, json, re
from pathlib import Path, PurePosixPath

SOURCE = '64964b539a207e5727d8a537e03887bcaea5f197'
PREFIX = 'crates/dive-pluscal/acyclic-procedure-checksum-acceptance/'
OLD = 'crates/dive-pluscal/nested-procedure-checksum-acceptance/'
ELSIF = 'crates/dive-pluscal/p-elsif-checksum-acceptance/'

def need(condition, message):
    if not condition:
        raise ValueError(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def rel(name):
    path = PurePosixPath(name)
    need(not path.is_absolute() and '..' not in path.parts, 'unsafe path')
    return path.as_posix()

def verify(base, binaries=None):
    delivery = json.loads((base / 'delivery.json').read_bytes())
    need(delivery['source_commit'] == SOURCE and delivery['all_consumers_closed'] and
         delivery['family_certified'] is False and delivery['model_proved'] is False, 'qualification')
    archive = (base / rel(delivery['records_archive'])).read_bytes()
    need(sha(archive) == delivery['records_archive_sha256'], 'archive identity')
    records = {}
    for row in json.loads(gzip.decompress(archive)):
        name = rel(row['path'])
        need(name not in records, 'duplicate record')
        data = base64.b64decode(row['data'], validate=True)
        need(sha(data) == row['sha256'] == delivery['files_sha256'].get(name), 'record identity:' + name)
        records[name] = data
    need(set(records) == set(delivery['files_sha256']) and len(records) == delivery['records_count'] and
         sum(map(len, records.values())) == delivery['raw_bytes'], 'complete records')
    def doc(name):
        return json.loads(records[rel(name)])
    def packed(name):
        return gzip.decompress(records[rel(name)])
    producer = doc('final-runtime/producer.json')
    need(producer['source_commit'] == SOURCE and producer['source_committed'] and
         producer['input_count'] == len(producer['inputs']) == 15944, 'producer')
    def source(name):
        if name.startswith('@/'):
            return packed('external-reference-supplement/source/' + rel(name[2:]) + '.gz')
        return packed('final-runtime/source/' + rel(name) + '.gz')
    for name, digest in producer['inputs'].items():
        need(sha(source(name)) == digest, 'source archive:' + name)
    for folder, expected in [('literal-fixture-supplement', 43), ('external-reference-supplement', 208)]:
        index = doc(folder + '/index.json')
        need(len(index['inputs']) == expected, 'separate input census')
        for name, meta in index['inputs'].items():
            archive_name = folder + '/' + rel(meta['archive'])
            need(sha(records[archive_name]) == meta['archive_sha256'] and
                 sha(packed(archive_name)) == meta['sha256'], 'supplement input:' + name)
    need(delivery['binary_sha256'] == producer['binary_sha256'], 'binary authority')
    if binaries:
        for name, digest in delivery['binary_sha256'].items():
            need(sha((binaries / rel(name)).read_bytes()) == digest, 'binary identity:' + name)
    first = doc('first-ENOSPC-epoch.json')
    need(first['status'] == 'failed' and first['error'] == 'ENOSPC' and
         first['selected'] == 130 and first['native_credit'] is False and first['primary_credit'] is False,
         'failed capture retagged')
    need(len(first['files_sha256']) == 1965, 'failed capture inventory')
    for name, digest in first['files_sha256'].items():
        need(sha(records['primary-first-ENOSPC/' + rel(name)]) == digest, 'first capture bytes')
    campaign = doc('final-runtime/campaign.json'); commands = doc('final-runtime/commands.json')
    need(campaign['all_commands_closed'] and len(campaign['rows']) == len(commands) == 21 and
         [row['command'] for row in campaign['rows']] == commands, 'original command inventory')
    failed = {i for i, row in enumerate(campaign['rows']) if row['exit_code'] != 0}
    need(failed == {10,11,12,13,19,20}, 'original failed commands concealed')
    repair = doc('command-repairs/campaign.json'); planned = doc('command-repairs/repair-plan.json')
    need(repair['all_commands_closed'] and len(repair['rows']) == 6 and
         all(row['exit_code'] == 0 for row in repair['rows']), 'command repairs')
    need([row['command'] for row in repair['rows']] == doc('command-repairs/commands.json') and
         planned['source_unchanged'] and planned['original_failures_never_credited'] and
         sha(records['final-runtime/campaign.json']) == planned['original_campaign_sha256'], 'repair identity')
    for i, (original, fixed) in enumerate(zip([10,11,12,13], repair['rows'][:4])):
        before, after = commands[original], fixed['command']
        permitted = {2, before.index('--out') + 1}
        need(len(before) == len(after) and all(a == z for n,(a,z) in enumerate(zip(before,after))
             if n not in permitted) and after[2].endswith(PREFIX + ('compare.py' if i%2 == 0 else 'public-controls.py')),
             'repair exceeds script/output path')
    for index, count in [(4,21),(5,16)]:
        before = commands[19 if index == 4 else 20]; after = repair['rows'][index]['command']
        found = re.search(r"DIVE_BIN=('[^']+')", before[-1])
        need(found is not None, 'original binding authority')
        binary = ast.literal_eval(found[1])
        expected = before[-1].replace('],env=env))', ", '--dive-bin', " + repr(binary) + '],env=env))')
        need(before[:-1] == after[:-1] and after[-1] == expected,
             'repair exceeds explicit binary binding')
        log = records[f'command-repairs/campaign-{index:02}.log']
        need(b'workers [1, 4, 8]' in log and
             f'gate: {count} of {count} models run'.encode() in log and b'gate: passed' in log and
             len(re.findall(rb'\[\d+/\d+\] agree', log)) == count, 'actual conformance gate census')
    gate = doc('full-gate/report.json')
    need(gate['source_commit'] == SOURCE and gate['passed'] and gate['all_stages_closed'] and
         gate['all_stages'] == len(gate['completed']) == 53 and
         all(row['exit_code'] == 0 for row in gate['completed']), 'full gate')
    need([row['command'] for row in gate['completed']] == doc('full-gate/commands.json'), 'gate command inventory')
    for row in gate['completed']:
        need(sha(records['full-gate/' + rel(row['log'])]) == row['log_sha256'], 'gate log identity')
    inventory = re.search(rb'(\d+) tests run:\s*(\d+) passed,\s*(\d+) skipped', records['full-gate/47.log'])
    need(inventory and inventory[1] == inventory[2] == b'1197' and inventory[3] == b'0', 'actual complete nextest')

    def raw(folder, scope, total, expected):
        report = doc(folder + '/report.json')
        need(report['selected'] == len(report['rows']) == total and report['summary'] == expected and
             report['family_certified'] is False and report['native_binary_sha256'] ==
             producer['binary_sha256']['context'], 'raw census:' + folder)
        primary = json.loads(source(scope + 'primary/manifest.json'))
        references = {row['id']: row for row in primary['rows']}
        need(set(references) == {row['id'] for row in report['rows']}, 'raw complete selection')
        for row in report['rows']:
            output = row.get('document')
            if row['outcome'] == 'command-rejection-uncredited':
                need(output is None, 'invalid command credited')
                continue
            need(output['artifact_admitted'] is False and output['model_proved'] is False and
                 output.get('exhaustive', False) is False, 'raw false proof')
            if row['outcome'] in ['exact-raw-ast', 'primary-invalid-raw-uncredited']:
                checksum = output['checksum']['Ok']
                need(checksum['serialized_ast'].encode() == source(scope + 'primary/' + row['id'] +
                     '.translated.ast') and checksum['checksum'] == references[row['id']]['stages']['translated']['crc'],
                     'raw exact header bytes/CRC')
            else:
                need(row['outcome'] in ['named-refusal', 'rejection-accounted'], 'raw unknown disposition')
    raw('final-runtime/raw-new', PREFIX, 130,
        {'exact-raw-ast':104,'named-refusal':18,'rejection-accounted':6,'command-rejection-uncredited':2})
    raw('final-runtime/raw-legacy-elsif32', ELSIF, 32, {'exact-raw-ast':32})
    raw('final-runtime/raw-legacy-original180', OLD, 180,
        {'exact-raw-ast':137,'named-refusal':24,'rejection-accounted':17,'primary-invalid-raw-uncredited':2})
    raw('final-runtime/raw-legacy-tail4', OLD+'while-tail/',4,{'exact-raw-ast':4})
    raw('final-runtime/raw-legacy-lines8', OLD+'while-lines/',8,{'exact-raw-ast':8})
    raw('command-repairs/raw-legacy-branch4','@/branch-study/',4,{'exact-raw-ast':4})
    raw('command-repairs/raw-legacy-reachability8','@/branch-reachability/',8,{'exact-raw-ast':8})

    def public(folder, scope, total, expected):
        report = doc(folder + '/report.json')
        need(report['selected'] == len(report['rows']) == total and report['summary'] == expected and
             report['model_proved'] is False and report['family_certified'] is False and
             report['native_binary_sha256'] == producer['binary_sha256']['dive'], 'public census:' + folder)
        references = json.loads(source(scope + 'primary/manifest.json'))
        ids = {row['id'] for row in references['rows']}
        need(len({row['id'] for row in report['rows']}) == total and
             {(row['source_id'], row['profile']) for row in report['rows']} ==
             {(name, profile) for name in ids for profile in ['dive', 'tlc']},
             'public complete paired selection')
        for row in report['rows']:
            stream = packed(folder + '/' + row['id'] + '.native.stdout.gz')
            need(sha(stream) == row['native']['stdout_sha256'], 'public raw stream')
            if row['outcome'] == 'command-rejection-uncredited':
                need(not stream and row['native']['exit_code'] == 2, 'command invalid credited')
                continue
            output = json.loads(stream)
            need(output['profile'] == row['profile'] and output['model_proved'] is False and
                 output['exhaustive'] is False and output['result'] != 'internal-error' and
                 output['exit_code'] == row['native']['exit_code'], 'public claims')
            if row['outcome'] == 'admitted-exact-cst-sany-validator':
                need(output['accepted'] and output['result'] == 'replacement' and output['exit_code'] == 0 and
                     output['algorithm_checksum_verified'] and output['body_checksum_verified'], 'admission')
                need(output['translation_options'] == output['option_context']['effective'] and
                     output['requested_translation_options'] == output['option_context']['requested'], 'option context')
                need(output['replacement']['algorithm_checksum']['serialized_ast'].encode() ==
                     source(scope + 'primary/' + row['source_id'] + '.translated.ast'), 'public stored header bytes')
                left = json.loads(packed(folder + '/' + row['id'] + '.native-cst.stdout.gz'))
                right = json.loads(packed(folder + '/' + row['id'] + '.primary-cst.stdout.gz'))
                need(left['accepted'] and right['accepted'] and left['tree'] == right['tree'], 'whole CST')
                for tag in ['native', 'primary']:
                    need(b'Semantic errors:' not in packed(folder + '/' + row['id'] + '.' + tag + '-sany.stdout.gz'), 'SANY')
                need('NO_DIVERGENCE' in row['validator']['configured'], 'Validator')
            else:
                need(not output['accepted'] and output['result'] in ['unsupported', 'rejected'] and
                     output['exit_code'] in [1, 3] and 'replacement' not in output, 'unadmitted artifact')
    public('final-runtime/public-new', PREFIX,260,
           {'admitted-exact-cst-sany-validator':208,'named-public-refusal':36,'rejection-accounted':12,'command-rejection-uncredited':4})
    public('final-runtime/public-legacy-elsif32',ELSIF,64,{'admitted-exact-cst-sany-validator':64})
    public('final-runtime/public-legacy-original180',OLD,360,
           {'admitted-exact-cst-sany-validator':274,'named-public-refusal':36,'rejection-accounted':38,'invalid-primary-uncredited':12})
    public('final-runtime/public-legacy-tail4',OLD+'while-tail/',8,{'admitted-exact-cst-sany-validator':8})
    public('final-runtime/public-legacy-lines8',OLD+'while-lines/',16,{'admitted-exact-cst-sany-validator':8,'named-public-refusal':8})
    public('command-repairs/public-legacy-branch4','@/branch-study/',8,{'admitted-exact-cst-sany-validator':8})
    public('command-repairs/public-legacy-reachability8','@/branch-reachability/',16,{'admitted-exact-cst-sany-validator':16})

    models = doc('final-runtime/models-new/report.json')
    need(models['selected'] == len(models['rows']) == 124 and models['worker_store_controls'] == 1488 and
         not models['family_certified'], 'model census')
    for row in models['rows']:
        need(len(row['settings']) == 12 and row['outcome'] == 'finite-model-match', 'finite model')
        need({(setting['profile'], setting['workers'], setting['store']) for setting in row['settings']} ==
             {(profile, width, store) for profile in ['dive', 'tlc'] for width in [1, 4, 8]
              for store in ['fp', 'exact']}, 'complete worker/store/profile matrix')
        stream = packed('final-runtime/models-new/' + row['primary']['stdout']['path'])
        counts = re.findall(rb'([0-9,]+) states generated, ([0-9,]+) distinct states found, ([0-9,]+) states left on queue\.', stream)
        need(len(counts) == 1 and all(row['primary_counts'][key] == int(value.replace(b',', b''))
             for key, value in zip(['generated', 'distinct', 'left_on_queue'], counts[0])), 'primary counts')
        for setting in row['settings']:
            output = json.loads(packed('final-runtime/models-new/' + setting['capture']['stdout']['path']))
            violation = row['id'].endswith('.violation')
            need(output['verdict'] == ('invariant-violation' if violation else 'success') and
                 all(output['statistics'][key] == value for key, value in row['primary_counts'].items()) and
                 setting['capture']['exit_code'] == ((12 if setting['profile'] == 'tlc' else 10) if violation else 0), 'model class/count/exit')
    replay = doc('final-runtime/replay-new/report.json')
    need(replay['selected'] == len(replay['rows']) == 620 and replay['qualified_canonical_is_not_tlc_validation'] and
         replay['summary'] == {'dive.valid': 62, 'dive.invalid': 248, 'tlc.unsupported': 186, 'tlc.invalid': 124}, 'replay census')
    strict = canonical = 0
    for row in replay['rows']:
        output = json.loads(packed('final-runtime/replay-new/' + row['id'] + '.stdout.gz'))
        need(output['result'] == row['actual_result'] and ('.' + row['profile'] + '.') in row['id'] and
             row['exit_code'] == {'valid': 0, 'invalid': 1, 'unsupported': 3}[output['result']], 'replay claims/profile')
        if row['control'] != 'original':
            need(output['result'] != 'valid', 'mutant falsely valid')
        elif output['result'] == 'valid':
            if row['profile'] == 'tlc':
                strict += 1
            else:
                canonical += 1
    need(strict == 0 and canonical == 62, 'strict saved-frame credit')
    blocks = re.findall(rb'```tla\n(.*?)\n```', source('docs/pluscal-acyclic-procedure-checksums.md'), re.S)
    need(len(blocks) == 2 and records['markdown-guide-controls/guide/cases/AcyclicProcedureChecksum_P.tla'] ==
         blocks[0] + b'\n' and records['markdown-guide-controls/guide/cases/AcyclicProcedureChecksum_C.tla'] ==
         b'---- MODULE AcyclicProcedureChecksum_C ----\nEXTENDS Integers, Sequences, TLC\n' + blocks[1] + b'\n====\n',
         'executed Markdown source derivation')
    for folder in ['final-runtime/guides-final', 'markdown-guide-controls/native']:
        guides = doc(folder + '/report.json')
        need(guides['selected'] == len(guides['rows']) == 20 and all(row['outcome'] == 'admitted-no-model-proof'
             for row in guides['rows']), 'guide controls')
        for row in guides['rows']:
            stream = packed(folder + '/' + row['id'] + '.stdout.gz')
            output = json.loads(stream)
            need(sha(stream) == row['stdout_sha256'] and output['model_proved'] is False and output['exhaustive'] is False and
                 output['accepted'] is True and output['result'] == 'replacement' and output['body_checksum_verified'], 'guide claims')
            name, profile, mode = row['id'].split('.')
            need(output['profile'] == ('tlc+overrides' if profile == 'tlc' and mode.endswith('body') else profile), 'guide profile')
            if folder.startswith('markdown'):
                input_bytes = records['markdown-guide-controls/guide/cases/' + name + '.tla']
                expected_ast = records['markdown-guide-controls/guide/primary/' + name + '.translated.ast']
                references = doc('markdown-guide-controls/guide/primary/manifest.json')
            else:
                input_bytes = source(PREFIX + 'guide/cases/' + name + '.tla')
                expected_ast = source(PREFIX + 'guide/primary/' + name + '.translated.ast')
                references = json.loads(source(PREFIX + 'guide/primary/manifest.json'))
            need(sha(input_bytes) == row['source_sha256'], 'guide source')
            reference = next(item for item in references['rows'] if item['id'] == name)
            if mode.endswith('body'):
                need(output['algorithm_checksum_verified'] is False, 'body-only false full claim')
            else:
                need(output['algorithm_checksum_verified'] and output['replacement']['algorithm_checksum']['serialized_ast'].encode() ==
                     expected_ast and output['replacement']['algorithm_checksum']['checksum'] == reference['header'][0], 'guide full AST/CRC')
    crc = doc('final-runtime/crc/report.json')
    need(crc['selected'] == len(crc['rows']) == 24 and all(row['model_proved'] is False and row['outcome'] in
         ['valid-unchanged', 'corruption-refused'] for row in crc['rows']), 'CRC controls')
    forgery = doc('final-runtime/forgeries/report.json')
    need(forgery['selected'] == 28 and forgery['genuine_controls'] == 4 and forgery['forged_controls'] == 24 and
         all(row['exit_code'] == int(row['mutation'] != 'baseline') for row in forgery['rows']), 'forgery controls')
    mutations = doc('primary-forgery-driver-replay/report.json')
    need(mutations['selected'] == len(mutations['rows']) == 8 and all(row['exit_code'] == int(row['control'] != 'baseline')
         for row in mutations['rows']), 'primary integrity mutations')
    return {'records': len(records), 'source_inputs': 15944, 'gate_tests': int(inventory[1]),
            'model_settings': 1488, 'strict_saved_frame_valid': 0, 'family_certified': False}

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--base', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--binaries', type=Path)
    args = parser.parse_args()
    try:
        print(json.dumps(verify(args.base, args.binaries), sort_keys=True))
    except (ValueError, KeyError, TypeError, AssertionError, FileNotFoundError) as error:
        raise SystemExit('Acyclic procedure delivery verification failed: ' + str(error))
