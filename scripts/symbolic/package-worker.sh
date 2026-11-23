#!/usr/bin/env bash
# Assemble the native release worker in a new output directory.
set -euo pipefail
[[ $# == 2 ]] || { echo 'expected: EXPORTER NEW_DIRECTORY' >&2; exit 2; }
exporter=$(realpath "$1")
destination=$(realpath -m "$2")
source_dir=$(cd "$(dirname "$0")" && pwd)
arch=$(uname -m)
case "$arch" in
    x86_64) digest=40281f47aa93fc96fb4aa51a4ff74284c07ab5304a1de3e8d095e5dba099bb72 ;;
    aarch64) digest=00043bdcc556f7c8784b7b621f8bde55aa3177f1a809af4372ca72e3828c887d ;;
    *) echo "unsupported release architecture: $arch" >&2; exit 2 ;;
esac
[[ ! -e $destination && -x $exporter ]]
build=$(mktemp -d)
trap 'rm -rf "$build"' EXIT
cd "$build"
fetch() {
    curl --retry 3 -fLsS "$1" -o "$2"
    echo "$3  $2" | sha256sum -c -
}
fetch "https://github.com/astral-sh/python-build-standalone/releases/download/20260203/cpython-3.11.14%2B20260203-$arch-unknown-linux-gnu-pgo%2Blto-full.tar.zst" python.tar.zst "$digest"
tar xf python.tar.zst python/install python/licenses
mkdir worker
mv python/install worker/python
mv python/licenses worker/licenses
py="$build/worker/python/bin/python3.11"
# Use the release interpreter and its bundled setuptools to build the pure packages.
"$py" -Es -m pip --isolated install --no-deps --no-build-isolation --require-hashes -r "$source_dir/release-requirements.txt"
fetch https://codeload.github.com/ivmai/cudd/tar.gz/refs/tags/cudd-3.0.0 cudd.tgz 5fe145041c594689e6e7cf4cd623d5f2b7c36261708be8c9a72aed72cf67acce
tar xzf cudd.tgz
(cd cudd-cudd-3.0.0 && ./configure CFLAGS="-O2 -fPIC -std=c99" && make -j4)
fetch https://files.pythonhosted.org/packages/source/d/dd/dd-0.6.0.tar.gz dd.tgz 4baadadc9b2ebf6136a5b84dc51a43cec5fe91203286cd377e1e093358cdffcd
tar xzf dd.tgz
(cd dd-0.6.0 && "$py" -Es setup.py --cudd="$build/cudd-cudd-3.0.0" bdist_wheel)
"$py" -Es -m pip --isolated install --no-deps dd-0.6.0/dist/*.whl
"$py" -Es -c 'from dd import cudd; assert cudd.BDD().true.dag_size == 1'
install -m 644 cudd-cudd-3.0.0/LICENSE worker/licenses/LICENSE.cudd
install -m 644 "$source_dir"/{auto,bounded_path,check,compiled,engine,finite_relation,finite_values,induction,obligation_receipt,occupancy,occupancy_predicate,occupancy_source,packed_codegen,property_proof,worker_process}.py worker/
install -m 644 "$source_dir/packed_worker.rs.in" worker/
install -m 755 "$exporter" worker/symbolic_input
# Trusted callers can issue verify+write in one process when this capability is installed.
mkdir worker/capabilities
touch worker/capabilities/named-predicate-repair-v1
mkdir -p "$(dirname "$destination")"
mv -T worker "$destination"
