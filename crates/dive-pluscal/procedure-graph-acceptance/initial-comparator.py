#!/usr/bin/env python3
"""Compare native bounded process artifacts, retaining every selected stage/result.

Significant-token equivalence is not pretty-width, checksum, file-rewriting or
mapping-format certification. Invalid primary artifacts never earn matches.
"""
import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

JAR_SHA = '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'

def sha(data):
    return hashlib.sha256(data).hexdigest()

def execute(native, source, args=()):
    result = subprocess.run([str(native), *args], input=source, capture_output=True, timeout=20)
    if result.returncode != 0:
        raise ValueError('native failed: ' + result.stderr.decode(errors='replace'))
    return json.loads(result.stdout)

def verify(directory, fields):
    for stream in ('stdout', 'stderr'):
        data = gzip.decompress((directory / fields[stream]).read_bytes())
        if sha(data) != fields[stream + '_sha256']:
            raise ValueError('raw primary transcript integrity mismatch')

def source_offset(source, position):
    lines=source.decode().splitlines(keepends=True)
    index=position['line']; target=position['column']
    if not 0<=index<len(lines):raise ValueError('reference source line outside input')
    base=sum(len(line.encode()) for line in lines[:index]);column=0
    for char in lines[index]:
        if column==target:return base
        if column>target:raise ValueError('reference column inside expanded tab')
        column+=8-column%8 if char=='\t' else 1;base+=len(char.encode())
    if column==target:return base
    raise ValueError('reference source column outside input')

def location(label):
    call=label['position']; definition=label.get('macro_definition_position')
    if definition is not None:
        return f"line {definition['line']}, column {definition['column']} of macro called at line {call['line']}, column {call['column']}"
    return f"line {call['line']}, column {call['column']}"

def outcome(row, document, source, directory, native):
    if document.get('model_proved') is not False or document.get('exhaustive') is not False:
        return {'outcome':'disagree','reason':'translation must never claim model proof'}
    if 'artifact' not in document:
        if document.get('code') not in {'DIVE-P0001','DIVE-P0002','DIVE-P0003','DIVE-P0004','DIVE-P0040','DIVE-P0090'} or 'error' not in document:
            return {'outcome':'disagree','reason':'internal/unknown native failure'}
        known={'DIVE-P0001':'Lexical','DIVE-P0002':'Syntax','DIVE-P0003':'Expression','DIVE-P0004':'Label','DIVE-P0040':'Resource','DIVE-P0090':'Unsupported'}
        error=document['error'];span=error.get('span',{})
        if document.get('stage') not in ('Source','Normalization','Generation') or error.get('code')!=known[document['code']] or document.get('accepted') is True:
            return {'outcome':'disagree','reason':'failure stage/category contradicts its disposition'}
        a,b=span.get('start',-1),span.get('end',-1)
        if not 0<=a<=b<=len(source):return {'outcome':'disagree','reason':'failure source extent invalid'}
        source[:a].decode();source[:b].decode()
        if row['primary']['status'] == 'oracle-error':
            return {'outcome':'oracle-error','reason':'primary translator did not return an interpretable result','native_code':document['code']}
        if document['code'] in {'DIVE-P0090','DIVE-P0040'}:
            return {'outcome':'native-refusal','reason':document['error']['message'],'code':document['code']}
        if row['primary']['status'] == 'rejected':
            return {'outcome':'rejection-match','code':document['code']}
        if row.get('sany',{}).get('status') == 'rejected':
            return {'outcome':'invalid-reference-artifact','reason':document['error']['message']}
        return {'outcome':'disagree','reason':'native rejected primary-valid translation','native':document}
    if document.get('stage') != 'artifact' or row['primary']['status'] != 'translated':
        return {'outcome':'disagree','reason':'native artifact without primary translation'}
    if document.get('accepted') is not True or document.get('exit_code') != 0:
        return {'outcome':'disagree','reason':'artifact disposition/exit contradicts artifact claim'}
    artifact = document['artifact']
    data = artifact['tla'].encode()
    if not data or not artifact.get('actions') or not artifact.get('mappings'):
        return {'outcome':'disagree','reason':'empty native artifact/evidence'}
    for mapping in artifact['mappings']:
        for key, extent in (('generated',data),('original',source)):
            a,b = mapping[key]['start'],mapping[key]['end']
            if not 0 <= a <= b <= len(extent):
                return {'outcome':'disagree','reason':'source-map extent invalid'}
            extent[:a].decode(); extent[:b].decode()
    actual_tokens = execute(native, data, ('--tokens',))['tokens']
    if actual_tokens != document.get('tokens'):
        return {'outcome':'disagree','reason':'native token metadata does not describe artifact body'}
    config = (directory / row['config']['path']).read_text() if 'config' in row else ''
    expected_config = 'CONSTANT defaultInitValue = defaultInitValue\n' if 'CONSTANT defaultInitValue = defaultInitValue' in config else ''
    if artifact.get('configuration') != expected_config:
        return {'outcome':'disagree','reason':'required default model-value configuration differs'}
    inserted = artifact.get('inserted_labels')
    if not isinstance(inserted,list):return {'outcome':'disagree','reason':'missing inserted-label metadata'}
    for label in inserted:
        a,b=label['source']['start'],label['source']['end']
        if not 0<=a<b<=len(source) or label['name'] not in artifact['actions']:
            # Branch-internal labels are refused by this bounded generator.
            return {'outcome':'disagree','reason':'inserted label metadata invalid'}
        source[:a].decode();source[:b].decode()
        definition=label.get('macro_definition'); position=label.get('macro_definition_position')
        if (definition is None)!=(position is None):return {'outcome':'disagree','reason':'incomplete macro source origin'}
        if definition is not None:
            a,b=definition['start'],definition['end']
            if not 0<=a<b<=len(source):return {'outcome':'disagree','reason':'macro definition outside original source'}
            source[:a].decode();source[:b].decode()
            offset=source_offset(source,{'line':position['line']-1,'column':position['column']-1})
            if not a<=offset<b:return {'outcome':'disagree','reason':'macro definition position outside its source'}
    normalized=row['normalized_reference_adapter']
    verify(directory,normalized)
    adapter=(directory/normalized['document']['path']).read_bytes()
    if sha(adapter)!=normalized['document']['sha256']:raise ValueError('normalization adapter integrity mismatch')
    adapter=json.loads(adapter)
    if [label['name'] for label in inserted]!=adapter.get('inserted') or [location(label) for label in inserted]!=adapter.get('locations') or bool(expected_config)!=adapter.get('default_initialization'):
        return {'outcome':'disagree','reason':'normalized inserted label/name/source position/default binding mismatch'}
    processes = adapter['ast'].get('procs', [])
    metadata = artifact.get('processes')
    if not isinstance(metadata,list) or len(metadata)!=len(processes):
        return {'outcome':'disagree','reason':'process normalization/storage cardinality differs'}
    for process, meta in zip(processes,metadata):
        if meta.get('name')!=process['name'] or meta.get('binding_kind')!=('Equal' if process['isEq'] else 'Member') or meta.get('local_variables')!=[decl['var'] for decl in process['decls']]:
            return {'outcome':'disagree','reason':'process normalization/storage metadata differs'}
        for key in ('source','id_source'):
            a,b=meta[key]['start'],meta[key]['end']
            if not 0<=a<b<=len(source):return {'outcome':'disagree','reason':'process original source extent invalid'}
            source[:a].decode();source[:b].decode()
        origin=process['origin']
        start,end=source_offset(source,origin['begin']),source_offset(source,origin['end'])
        # The reference Process.origin starts at `process`; our source AST
        # span includes the preceding `fair`/`fair +` declaration qualifier.
        # Recover that exact original qualifier, rather than accepting any
        # merely valid source extent containing the process.
        prefix=re.search(rb'\bfair\s+(?:\+\s*)?$',source[:start])
        if prefix:start=prefix.start()
        extent=meta['source']
        if extent['start']!=start or extent['end']<end or source[end:extent['end']].strip() not in (b'',b';'):
            return {'outcome':'disagree','reason':'process source association differs from original normalized AST'}
        expected_id=[token['string'] for line in process['id']['tokens'] for token in line]
        extent=meta['id_source']
        actual_id=execute(native,source[extent['start']:extent['end']],('--tokens',))['tokens']
        reference_tokens=[token for line in process['id']['tokens'] for token in line]
        actual_id=[(word[1:-1] if word.startswith(chr(34)) and word.endswith(chr(34)) else None) if token['type']==3 else word for word,token in zip(actual_id,reference_tokens)] if len(actual_id)==len(reference_tokens) else actual_id
        if actual_id!=expected_id:
            return {'outcome':'disagree','reason':'process ID/domain source association differs'}
    actual_tokens = execute(native, data, ('--tokens',))['tokens']
    if actual_tokens != document.get('tokens'):
        return {'outcome':'disagree','reason':'native token metadata does not describe artifact body'}
    reference = (directory / row['translated']['path']).read_bytes()
    if sha(reference) != row['translated']['sha256']:
        raise ValueError('primary artifact integrity mismatch')
    lines = reference.decode().splitlines()
    start = next(i for i,line in enumerate(lines) if 'BEGIN TRANSLATION' in line)
    end = next(i for i,line in enumerate(lines[start+1:],start+1) if 'END TRANSLATION' in line)
    expected = execute(native, ('\n'.join(lines[start+1:end])+'\n').encode(), ('--tokens',))['tokens']
    declaration = re.search(r'^VARIABLES? +([^\n]+)', '\n'.join(lines[start+1:end]), re.M)
    expected_pc = bool(declaration and declaration.group(1).strip().split(',')[0].strip() == 'pc')
    if artifact.get('program_counter') is not expected_pc:
        return {'outcome':'disagree','reason':'program-counter metadata differs from primary artifact'}
    if expected_pc:
        names = re.findall(r'^(\w+)(?:\(self\))? *==', '\n'.join(lines[start+1:end]), re.M)
        names = [name for name in names if name not in ('vars','Init','Next','Spec','Termination','Terminating','ProcSet',*[p['name'] for p in processes],*[p['name'] for p in adapter['ast'].get('prcds',[])])]
        if artifact['actions'] != names:
            return {'outcome':'disagree','reason':'action metadata differs from primary artifact'}
    elif artifact['actions'] != ([p['body'][0]['label'] for p in processes] if processes else [adapter['ast']['body'][0]['label']]):
        return {'outcome':'disagree','reason':'pc-free action metadata must name every source loop'}
    if expected != document.get('tokens'):
        index = next((i for i,(a,b) in enumerate(zip(expected,document.get('tokens',[]))) if a != b), min(len(expected),len(document.get('tokens',[]))))
        return {'outcome':'disagree','reason':'significant token stream differs','index':index,'expected':expected[max(0,index-5):index+12],'actual':document.get('tokens',[])[max(0,index-5):index+12]}
    if row['sany']['status'] != 'accepted':
        return {'outcome':'invalid-reference-artifact','diagnostic_only':True,'admitted_artifact':False,'raw_tokens_equal':True}
    return {'outcome':'token-artifact-match','generated_sha256':sha(data),'action_count':len(artifact['actions']), 'program_counter':artifact['program_counter']}

def compare(args):
    manifest = json.loads(args.manifest.read_text())
    if manifest['oracle_sha256'] != JAR_SHA or not manifest['rows']:
        raise ValueError('wrong oracle pin/empty selection')
    selection = json.loads(args.inputs.parent.joinpath('selection.json').read_text())
    ids = [row['id'] for row in manifest['rows']]
    if len(set(ids)) != len(ids) or set(ids) != {row['id'] for row in selection['rows']} or set(ids) != {path.stem for path in args.inputs.glob('*.tla')}:
        raise ValueError('complete selection does not equal retained rows and inputs')
    if manifest['adapter_sha256'] != sha(Path(__file__).with_name('NormalizationProbe.java').read_bytes()): raise ValueError('normalized AST adapter provenance mismatch')
    if manifest['source_commit'] != '94d0c5070852b37b801bb8998c436383ab7ee37b' or not manifest.get('reason'):
        raise ValueError('oracle source/reason provenance mismatch')
    if manifest['capture_script_sha256'] != sha(Path(__file__).with_name('capture.py').read_bytes()):
        raise ValueError('capture script changed after recording primary evidence')
    rows=[]
    for row in manifest['rows']:
        source=(args.inputs/row['input']).read_bytes()
        if sha(source) != row['input_sha256']:
            raise ValueError('input integrity mismatch')
        verify(args.manifest.parent,row['primary'])
        if 'sany' in row: verify(args.manifest.parent,row['sany'])
        for name in ('translated','config'):
            if name in row and sha((args.manifest.parent/row[name]['path']).read_bytes()) != row[name]['sha256']:
                raise ValueError('artifact integrity mismatch')
        native=execute(args.native,source,row["options"])
        result=outcome(row,native,source,args.manifest.parent,args.native)
        rows.append({'id':row['id'],'primary':row['primary']['status'],'primary_sany':row.get('sany',{}).get('status'),**result})
    report={'schema_version':1,'stage':'procedure-token-artifacts','oracle_id':manifest['oracle_id'],'oracle_sha256':JAR_SHA,'manifest_sha256':sha(args.manifest.read_bytes()),'comparator_sha256':sha(Path(__file__).read_bytes()),'native_sha256':sha(args.native.read_bytes()),'model_proved':False,'pending':['unproved-overlapping-domains','name-renaming','procedures','unproved-macro-substitution','labels-inside-branches','historical-options','pretty-width','file-rewriting','checksums','mapping-format'],'selected':len(rows),'summary':dict(Counter(row['outcome'] for row in rows)),'rows':rows}
    args.output.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report['summary']))
    return 1 if report['summary'].get('disagree') else 0

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('manifest','inputs','native','output'):parser.add_argument('--'+name,type=Path,required=True)
    raise SystemExit(compare(parser.parse_args()))
