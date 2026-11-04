#!/usr/bin/env bash
# Build every package and collect the artifacts.
set -euo pipefail

OUT_DIR="${OUT_DIR:-./dist}"
VERSION=$(git describe --tags --always)
declare -a failed=()

log() { printf '\033[1m%s\033[0m %s\n' "==>" "$*"; }

mkdir -p "$OUT_DIR"
for pkg in packages/*/; do
  name=$(basename "$pkg")
  log "building $name ($VERSION)"
  if ! (cd "$pkg" && npm run build --silent > /dev/null 2>&1); then
    failed+=("$name")
    continue
  fi
  cp -r "$pkg/dist" "$OUT_DIR/$name"
done

if [[ ${#failed[@]} -gt 0 ]]; then
  echo "failed: ${failed[*]}" >&2
  exit 1
fi

cat <<MANIFEST > "$OUT_DIR/manifest.txt"
version: $VERSION
built: $(date -u +%Y-%m-%dT%H:%M:%SZ)
MANIFEST

case "$1" in
  --upload) rsync -az "$OUT_DIR/" "deploy@example.com:/srv/releases/$VERSION" ;;
  *) echo "done: $OUT_DIR" ;;
esac
