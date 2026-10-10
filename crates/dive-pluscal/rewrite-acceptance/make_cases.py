#!/usr/bin/env python3
"""Freeze C/P rewrite inputs before native block/checksum implementation."""
from pathlib import Path
import hashlib,json
BASE=Path(__file__).resolve().parent
BASE.joinpath('cases').mkdir(exist_ok=True)
rows=[]
def add(name,source,mode='noop'):
 data=source.encode();p=BASE/'cases'/(name+'.tla');p.write_bytes(data)
 rows.append({'id':name,'input':p.name,'input_sha256':hashlib.sha256(data).hexdigest(),'mode':mode})
for syntax in ['C','P']:
 alg='--algorithm A { variable x=0; L:x:=1; E:skip; }' if syntax=='C' else '--algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm;'
 plain='---- MODULE Rewrite_'+syntax+' ----\nEXTENDS Integers\n(* '+alg+' *)\n\\* preserved footer\n====\n'
 for name,source in [
  ('fresh',plain),('crlf',plain.replace('\n','\r\n')),('cr',plain.replace('\n','\r')),('no-final-newline',plain.rstrip('\n')),
  ('unicode-outside',plain.replace('EXTENDS','\\* λ🙂 preface\nEXTENDS').replace('footer','footer λ🙂')),
  ('blank-lines',plain.replace('(* ','\n\n(* ').replace('====','\n====\n')),
  ('comment-tail',plain.replace(' *)',' *) \\* trailing')),
  ('code-tail',plain.replace(' *)',' *) Z == 1')),
  ('tab-prefix',plain.replace('(* ','\t(* ')),('nested-comment',plain.replace('L:x','(* nested *) L:x')),
  ('string-marker',plain.replace('EXTENDS Integers','EXTENDS Integers\nString == "BEGIN TRANSLATION"')),
  ('no-algorithm',plain.replace(alg,'')),('unterminated',plain.replace(' *)','')),
  ('fair',plain.replace('--algorithm','--fair algorithm')),
  ('unsupported-procedure',plain.replace(alg,'--algorithm A { procedure R() { L:return; }; { E:call R(); } }')),
 ]:add(syntax+'_'+name,source)
 for header in [
  '\\* BEGIN TRANSLATION','\\* BEGIN  TRANSLATION',
  '\\* BEGIN TRANSLATION (chksum(pcal) \\in STRING /\\ chksum(tla) \\in STRING)',
  '\\* BEGIN TRANSLATION (chksum(pcal) = "ffffffff" /\\ chksum(tla) = "ffffffff")',
  '\\* BEGIN TRANSLATION (CHECKSUM(PLUSCAL) = "0" /\\ CHECKSUM(TLA+) = "0") tail',
  '\\* BEGIN TRANSLATION (chksum(pcal) = "" /\\ chksum(tla) = "")',
  '\\* BEGIN TRANSLATION strange header',
  ]:
  index=len([r for r in rows if r['id'].startswith(syntax+'_header')])
  source=plain.replace('\\* preserved footer',header+'\n\\* edited old body\nold == 23\n\\* END TRANSLATION \\* retained tail\n\\* preserved footer')
  for mode in ['noop','cancel','generate','ignore']:add(syntax+'_header'+str(index)+'_'+mode,source,mode)
 for name,source in [
  ('begin-only',plain.replace('\\* preserved footer','\\* BEGIN TRANSLATION\nold == 1')),
  ('end-only',plain.replace('\\* preserved footer','\\* END TRANSLATION')),
  ('nested-markers',plain.replace('\\* preserved footer','\\* BEGIN TRANSLATION\n\\* BEGIN TRANSLATION\nold == 1\n\\* END TRANSLATION\n\\* END TRANSLATION')),
  ('marker-in-block',plain.replace('\\* preserved footer','(* BEGIN TRANSLATION\nold == 1\nEND TRANSLATION *)')),
  ('end-before-begin',plain.replace('\\* preserved footer','\\* END TRANSLATION\n\\* BEGIN TRANSLATION\nold == 1\n\\* END TRANSLATION')),
 ]:add(syntax+'_'+name,source)
(BASE/'selection.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'rows':rows},indent=2)+'\n')
print('Frozen',len(rows),'rewrite sources/options')
