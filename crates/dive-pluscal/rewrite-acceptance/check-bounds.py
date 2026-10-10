#!/usr/bin/env python3
"""Deterministic byte-budget and malformed UTF-8-source-span editor controls."""
import argparse,hashlib,importlib.util,json,random,subprocess
from pathlib import Path
BASE=Path(__file__).resolve().parent
def sha(data):return hashlib.sha256(data).hexdigest()
def main(args):
 spec=importlib.util.spec_from_file_location('owned',BASE/'compare.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);randomizer=random.Random(172);base='---- MODULE R ----\nEXTENDS Integers\n(* --algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)\n\\* λ🙂 footer\n====\n';rows=[]
 for index in range(512):
  source=base;body='x == '+str(index)+'\n';normalize=index%2==0
  if index%3==0:
   position=randomizer.randrange(len(source)+1);source=source[:position]+randomizer.choice(['(*','*)','\\* BEGIN TRANSLATION\n','🙂','\r\n','\x00'])+source[position:]
  maximum=randomizer.choice([0,len(source.encode())-1,len(source.encode()),len(source.encode())+128,4096]);p,doc=m.execute(args.example,{'source':source,'body':body,'normalize':normalize,'max_bytes':maximum});assert m.integrity(doc,p.returncode,'source-editor')
  if doc['accepted']:
   assert len(doc['plan']['module_source'].encode())<=maximum and m.plan_integrity(doc['plan'],source,body),(index,doc);outcome='bounded-replacement'
  else:
   span=doc['diagnostic']['span'];assert 0<=span['start']<=span['end']<=len(source.encode());source.encode()[:span['start']].decode();source.encode()[:span['end']].decode();assert doc['diagnostic']['code']!='Internal';outcome='named-failure'
  rows.append({'index':index,'input_sha256':sha(json.dumps({'source':source,'body':body,'normalize':normalize,'max_bytes':maximum}).encode()),'outcome':outcome})
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'comparator_sha256':sha((BASE/'compare.py').read_bytes()),'example_sha256':sha(args.example.read_bytes()),'controls':len(rows),'seed':172,'family_certified':False,'rows':rows};(BASE/'bounds-controls.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed all512 deterministic byte-budget/source-span controls')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--example',type=Path,required=True);main(p.parse_args())
