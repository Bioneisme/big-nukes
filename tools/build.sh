#!/bin/bash
# Checks and packages the mod on Linux or WSL.
#   tools/build.sh check [lines]    cargo check of the game with the mod applied
#   tools/build.sh package          dist/big_nukes_native.hnmod: the whole mod in one file, the content mod
#                                   (dist/big_nukes.hmod) inside it as data/big_nukes-content.hmod
#
# HORIZON_BUILD (default ~/horizon-build) holds a clone of horizon-mods in
# horizon-mods/, and its own rustup, cargo and uv, so nothing needs installing system-wide.
set -u
B=${HORIZON_BUILD:-$HOME/horizon-build}
SRC=$(cd "$(dirname "$0")/.." && pwd)
ID=big_nukes_native
export RUSTUP_HOME=$B/rustup CARGO_HOME=$B/cargo PATH=$B/cargo/bin:$B/uv:$B/tools/bin:$PATH
export UV_PYTHON_INSTALL_DIR=$B/python UV_CACHE_DIR=$B/uv-cache
export NATIVE_NO_DEFAULT_FEATURES=1 CARGO_BUILD_JOBS=4
DATA=$SRC/mixins/$ID/data/big_nukes-content.hmod
mkdir -p "$SRC/dist" "$(dirname "$DATA")"
# The content mod, built from content/ and carried in the bundle's data/ (native_data!).
content_mod() {
  rm -rf "$B/content"
  cp -r "$SRC/content" "$B/content"
  (cd "$B/content" && cargo horizon-mod build 2>&1 | tail -n 4) || exit 1
  cp "$B/content/target/horizon-mod/"*.hmod "$SRC/dist/big_nukes.hmod"
  cp "$SRC/dist/big_nukes.hmod" "$DATA"
}
case "${1:-check}" in
  package) content_mod ;;
  *) [ -f "$DATA" ] || content_mod ;;
esac
cd "$B/horizon-mods" || exit 1
mkdir -p "native/$ID"
rsync -a --delete --exclude target --exclude content --exclude dist --exclude docs --exclude tools --exclude .git "$SRC/" "native/$ID/"
PY="$(uv python find 3.12)"
case "${1:-check}" in
  check)
    "$PY" native/tools/native.py check "$ID" 2>&1 | grep -vE '^\s*(Compiling|Checking|Downloaded|Downloading|Updating|Blocking|Locking|Adding) ' | tail -n "${2:-120}"
    echo "exit: ${PIPESTATUS[0]}"
    ;;
  package)
    rm -f "native/target/$ID-"*.hnmod
    "$PY" native/tools/native.py package "$ID" 2>&1 | tail -n 5
    # native.py packs the .rs and .toml files only: the bundle's data/ goes in beside them, the same way
    # (fixed dates and modes, sorted), as mixins/<id>/data/... where the launcher reads a bundle's data.
    "$PY" - "native/target/$ID-"*.hnmod "$SRC/dist/$ID.hnmod" "$SRC/mixins/$ID" "$ID" <<'PYEOF'
import hashlib, json, pathlib, sys, zipfile
src, out, bundle, mod_id = sys.argv[1:5]
files = {}
with zipfile.ZipFile(src) as z:
    for name in z.namelist():
        files[name] = z.read(name)
for path in sorted(pathlib.Path(bundle, "data").rglob("*")):
    if path.is_file():
        files[f"mixins/{mod_id}/data/{path.relative_to(pathlib.Path(bundle, 'data')).as_posix()}"] = path.read_bytes()
with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as archive:
    for name, content in sorted(files.items()):
        info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        info.external_attr = 0o100644 << 16
        archive.writestr(info, content)
data = pathlib.Path(out).read_bytes()
print(json.dumps({"file": out, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()}, indent=2))
PYEOF
    ls -la "$SRC/dist"
    ;;
esac
