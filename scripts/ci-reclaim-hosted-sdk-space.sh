#!/usr/bin/env bash
# Ephemeral GitHub-hosted Linux jobs only; never a developer cleanup command.
set -euo pipefail

if [[ "${DIVE_CI_RUNNER_ENVIRONMENT:-}" != github-hosted || "${DIVE_CI_RUNNER_OS:-}" != Linux ]]; then
  echo "SDK cleanup requires an explicitly selected GitHub-hosted Linux runner" >&2
  exit 1
fi
: "${GITHUB_WORKSPACE:?missing workspace}" "${RUNNER_TEMP:?missing runner temp}"

df -h "$GITHUB_WORKSPACE" "$RUNNER_TEMP"
# Preserve the command's status and measure space even after a guarded failure.
trap 'status=$?; df -h "$GITHUB_WORKSPACE" "$RUNNER_TEMP" || true; exit "$status"' EXIT

sdk_paths=(/usr/local/lib/android /usr/share/dotnet /opt/ghc)
root_device=$(stat -c '%d' -- /)
# Validate every selected path before removing any of them.
for sdk in "${sdk_paths[@]}"; do
  if [[ -L "$sdk" ]]; then
    echo "Refusing SDK symlink: $sdk" >&2
    exit 1
  fi
  if [[ ! -e "$sdk" ]]; then continue; fi
  if [[ ! -d "$sdk" || "$(realpath -e -- "$sdk")" != "$sdk" ]]; then
    echo "Refusing non-directory or redirected SDK path: $sdk" >&2
    exit 1
  fi
  if [[ "$(stat -c '%d' -- "$sdk")" != "$root_device" ]] || mountpoint -q -- "$sdk"; then
    echo "Refusing mounted SDK root: $sdk" >&2
    exit 1
  fi
done
for sdk in "${sdk_paths[@]}"; do
  if [[ ! -e "$sdk" ]]; then continue; fi
  echo "Removing unused hosted SDK directory: $sdk"
  # Nested different-device mounts are never traversed. No tool/cache globbing.
  sudo -n rm --recursive --force --one-file-system -- "$sdk"
done
