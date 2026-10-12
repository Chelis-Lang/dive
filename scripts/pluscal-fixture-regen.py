#!/usr/bin/env python3
"""Regenerate missing PlusCal fixture oracle outputs from the pinned pcal.trans.

Usage: python3 -I scripts/pluscal-fixture-regen.py crates/dive-pluscal/tests/fixtures

For every fixture manifest row whose `translated.path` or
`normalized_reference_adapter.document.path` file is missing and whose primary
status is `translated`, runs the pinned pcal.trans (from $DIVE_CACHE_DIR) with
the row's options and writes the translated module, or a normalized document
{inserted, locations, default_initialization} built from `-reportLabels`.
"""
import glob, json, os, re, shutil, subprocess, sys, tempfile

cache = os.environ.get('DIVE_CACHE_DIR', os.path.expanduser('~/.cache/dive'))
java = glob.glob(os.path.join(cache, 'jre/*/*/Contents/Home/bin/java')) + glob.glob(os.path.join(cache, 'jre/*/*/bin/java'))
jar = os.path.join(cache, 'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar')


def run(src, options, report):
    d = tempfile.mkdtemp()
    name = os.path.basename(src)
    shutil.copy(src, os.path.join(d, name))
    extra = ['-reportLabels'] if report else []
    p = subprocess.run([java[0], '-cp', jar, 'pcal.trans', '-nocfg'] + extra + options + [name], cwd=d, capture_output=True, text=True)
    text = open(os.path.join(d, name)).read() if p.returncode == 0 else None
    shutil.rmtree(d)
    return p, text


def main(root):
    made = 0
    for manifest in glob.glob(os.path.join(root, '**/manifest.json'), recursive=True):
        base = os.path.dirname(manifest)
        for r in json.load(open(manifest)).get('rows', []):
            if r.get('primary', {}).get('status') != 'translated':
                continue
            src = os.path.join(os.path.dirname(base), 'cases', r['input'])
            if not os.path.exists(src):
                src = os.path.join(base, r['input'])
            options = r.get('options', [])
            t = r.get('translated')
            if isinstance(t, dict) and t.get('path') and not os.path.exists(os.path.join(base, t['path'])):
                p, text = run(src, options, False)
                if text is not None:
                    open(os.path.join(base, t['path']), 'w').write(text)
                    made += 1
            n = r.get('normalized_reference_adapter')
            if isinstance(n, dict) and isinstance(n.get('document'), dict):
                doc = os.path.join(base, n['document']['path'])
                if not os.path.exists(doc):
                    p, text = run(src, options, True)
                    if text is not None:
                        labels = re.findall(r'^  (\S+) at (line \d+, column \d+.*)$', p.stdout, re.M)
                        body = text[text.find('BEGIN TRANSLATION'):]
                        json.dump({'inserted': [l[0] for l in labels], 'locations': [l[1] for l in labels],
                                   'default_initialization': 'CONSTANT defaultInitValue' in body},
                                  open(doc, 'w'), indent=1)
                        made += 1
    print('generated', made)


if __name__ == '__main__':
    main(sys.argv[1])
