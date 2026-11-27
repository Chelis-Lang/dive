#!/usr/bin/env bash
# Package already-built native binaries; never rebuild or archive build trees.
set -euo pipefail

if [[ $# != 4 ]]; then
  echo "usage: $0 BINARY_DIR VERSION_TAG TARGET OUTPUT_DIR" >&2
  exit 2
fi

binary_dir=$(realpath "$1")
release_tag=$2
target=$3
[[ "$release_tag" =~ ^v[0-9]+[.][0-9]+[.][0-9]+(-[0-9A-Za-z.-]+)?$ ]]
case "$target" in
  x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu) ;;
  *) echo "unsupported release target: $target" >&2; exit 2 ;;
esac
mkdir -p "$4"
output_dir=$(realpath "$4")
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
for notice in \
  crates/dive-sema/bundled/README.md \
  crates/dive-sema/bundled/community-modules/LICENSE \
  crates/dive-sema/bundled/tlaps/LICENSE; do
  if [[ ! -f "$repo_root/$notice" ]]; then
    echo "missing release notice: $notice; bundled libraries must be present in the release source" >&2
    exit 1
  fi
done
name="dive-$release_tag-$target"
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
package="$stage/$name"
mkdir -p "$package/licenses"/{stdlib,pluscal,community-modules,tlaps}

install -D -m 644 "$repo_root/crates/dive-sema/bundled/dive/FramedFunctions.tla" \
    "$package/share/dive/tla/FramedFunctions.tla"
install -m 755 "$binary_dir/dive" "$binary_dir/tlc" "$package/"
strip --strip-unneeded "$package/dive" "$package/tlc"
install -m 644 "$repo_root/LICENSE" "$repo_root/README.md" "$package/"
install -m 644 "$repo_root/crates/dive-sema/stdlib/LICENSE" "$package/licenses/stdlib/"
install -m 644 "$repo_root/crates/dive-pluscal/src/translate/pcal/LICENSE" "$package/licenses/pluscal/"
install -m 644 "$repo_root/crates/dive-sema/bundled/README.md" "$package/licenses/"
install -m 644 "$repo_root/crates/dive-sema/bundled/community-modules/LICENSE" "$package/licenses/community-modules/"
install -m 644 "$repo_root/crates/dive-sema/bundled/tlaps/LICENSE" "$package/licenses/tlaps/"
tar -czf "$output_dir/$name.tar.gz" -C "$stage" "$name"
(
  cd "$output_dir"
  sha256sum "$name.tar.gz" > "$name.tar.gz.sha256"
)
