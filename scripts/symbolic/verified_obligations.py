"""Optional local authentication for exact, already discharged source obligations."""

from functools import cache
import hashlib
import hmac
import json
from pathlib import Path
import platform
import sys

from induction import write_atomic
from obligation_receipt import local_key, pack


def hash_files(paths):
    digests = []
    for path in map(Path, paths):
        with path.open('rb') as stream:
            digests.append(hashlib.file_digest(stream, 'sha256').hexdigest())
    return digests


class VerifiedObligations:
    def __init__(self, path, namespace, implementations):
        self.path = Path(path) if path else None
        self.secret = local_key() if self.path else None
        self.retained = self.local_hits = self.solved = 0
        self.prior = set()
        self.proved = set()
        self.prior_groups = {}
        self.groups = {}
        self.retained_groups = self.discharged_groups = 0
        self.context = None
        self.input_status = 'disabled'
        if self.secret is not None:
            try:
                self.context = hashlib.sha256(pack([
                    'source-obligation-cache-v1', namespace, platform.machine(), sys.version,
                    hash_files([__file__, sys.executable, *implementations]),
                ])).hexdigest()
            except (OSError, TypeError, ValueError):
                self.secret = None
                self.input_status = 'implementation-unavailable'
            else:
                self.prior, self.prior_groups, self.input_status = self.read()

    def tag(self, rows, groups):
        return hmac.digest(self.secret, pack([self.context, rows, groups]), 'sha256').hex()

    def read(self):
        try:
            with self.path.open('rb') as stream:
                raw = stream.read((8 << 20) + 1)
        except FileNotFoundError:
            return set(), {}, 'new'
        except OSError:
            return set(), {}, 'unreadable'
        try:
            obj = json.loads(raw) if len(raw) <= 8 << 20 else None
            if not isinstance(obj, dict) or obj.get('context') != self.context:
                return set(), {}, 'incompatible'
            rows, groups, tag = obj.get('obligations'), obj.get('groups'), obj.get('mac')
            if (not isinstance(rows, list) or len(rows) > 100_000 or not isinstance(tag, str)
                    or not tag.isascii() or len(tag) != 64
                    or any(not isinstance(row, str) or len(row) != 64 or not row.isascii() for row in rows)
                    or not isinstance(groups, dict) or len(groups) > 1024
                    or not hmac.compare_digest(self.tag(rows, groups), tag)):
                return set(), {}, 'unauthenticated'
            return set(rows), groups, 'authenticated'
        except (ValueError, RecursionError):
            return set(), {}, 'malformed'

    def unsat(self, inputs, discharge, *, remember_sat=False):
        if self.secret is None:
            self.solved += 1
            return discharge()
        yes = hashlib.sha256(pack([inputs, 'unsat'])).hexdigest()
        no = hashlib.sha256(pack([inputs, 'sat'])).hexdigest() if remember_sat else None
        for pool, counter in ((self.prior, 'retained'), (self.proved, 'local_hits')):
            if (yes in pool) != (no in pool):
                setattr(self, counter, getattr(self, counter) + 1)
                return yes in pool
        self.solved += 1
        result = discharge()
        token = yes if result else no
        if token is not None:
            self.proved.add(token)
        return result

    def group_key(self, inputs):
        return hashlib.sha256(pack(['action-group-v1', inputs])).hexdigest()

    def group(self, inputs):
        found = self.prior_groups.get(self.group_key(inputs)) if self.secret else None
        if found is not None:
            self.retained_groups += 1
        else:
            self.discharged_groups += 1
        return found

    def group_proved(self, inputs, evidence):
        if self.secret:
            self.groups[self.group_key(inputs)] = evidence

    def finish(self):
        # Replacing a competing result can lose useful hits. Each atomic file
        # is independently authenticated, so it cannot introduce a false hit.
        groups = self.prior_groups | self.groups
        rows = sorted(self.prior | self.proved)
        if self.secret is not None and (rows or groups) and len(rows) <= 100_000 and len(groups) <= 1024:
            try:
                write_atomic(self.path, pack(dict(context=self.context, obligations=rows, groups=groups, mac=self.tag(rows, groups))) + b'\n')
            except OSError:
                pass
        return dict(retained=self.retained, local_hits=self.local_hits, discharged=self.solved, input=self.input_status,
                    retained_groups=self.retained_groups, discharged_groups=self.discharged_groups)


def boolean_identity(manager):
    """Hash reduced ordered BDD contents, including each bit name and complement."""
    @cache
    def identity(node):
        if node.negated:
            data = b'N' + identity(~node)
        elif node == manager.true:
            data = b'T'
        else:
            data = b'B' + node.var.encode('ascii') + b'\0' + identity(node.low) + identity(node.high)
        return hashlib.sha256(data).digest()
    return identity
