#!/usr/bin/env python3
"""Check saved finite proof results, independent scopes and cold fallback."""

import json
import os
from pathlib import Path
from tempfile import TemporaryDirectory

from verified_obligations import VerifiedObligations


def check(root):
    path = root / 'proof.json'
    key = root / 'key'
    key.write_bytes(os.urandom(32))
    key.chmod(0o600)
    os.environ['DIVE_SYMBOLIC_PROOF_KEY'] = str(key)

    def proof(context='current'):
        return VerifiedObligations(path, [context], [__file__])

    first = proof()
    assert first.unsat(['premise', 'bad goal'], lambda: True)
    assert not first.unsat(['premise', 'reachable goal'], lambda: False, remember_sat=True)
    inputs = ['exact admitted action', 'all configured conjuncts', 'closed destination']
    assert first.group(inputs) is None
    first.group_proved(inputs, dict(action='Move', feasible=1))
    first.finish()

    def unexpected():
        raise AssertionError('reused proof must not run a solver')

    current = proof()
    assert current.unsat(['premise', 'bad goal'], unexpected)
    assert not current.unsat(['premise', 'reachable goal'], unexpected, remember_sat=True)
    assert current.group(inputs)['feasible'] == 1
    assert current.group([*inputs, 'new requirement']) is None
    assert current.finish()['retained'] == 2
    changed = proof('changed premises')
    assert changed.group(inputs) is None
    assert not changed.unsat(['premise', 'bad goal'], lambda: False)

    data = json.loads(path.read_text())
    data['mac'] = '0' * 64
    path.write_text(json.dumps(data))
    untrusted = proof()
    assert not untrusted.unsat(['premise', 'bad goal'], lambda: False)
    assert untrusted.group(inputs) is None
    assert untrusted.finish()['input'] == 'unauthenticated'


if __name__ == '__main__':
    with TemporaryDirectory(prefix='dive-obligations-') as directory:
        check(Path(directory))
