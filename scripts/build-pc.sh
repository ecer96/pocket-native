#!/bin/bash
# build-pc.sh <rom>... [--out dir]
#
# Builds Pocket Native and your games for this computer, to play in a window
# with the keyboard and mouse, inside a container (podman or docker). Leaves
# "Pocket Native (PC)" in the output directory, build-pc/ by default.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
roms=() out="$repo/build-pc"
while [ $# -gt 0 ]; do
    case "$1" in
        --out) out="$(realpath -m "$2")"; shift 2 ;;
        -h|--help) sed -n '2,6p' "$0"; exit 0 ;;
        *) roms+=("$(realpath -m "$1")"); shift ;;
    esac
done
if [ ${#roms[@]} -eq 0 ]; then
    echo "usage: scripts/build-pc.sh <rom>... [--out dir]"
    exit 2
fi
for rom in "${roms[@]}"; do
    [ -f "$rom" ] || { echo "no such file: $rom"; exit 2; }
done
engine=$(command -v podman || command -v docker) || { echo "the build needs podman or docker"; exit 1; }
image=pocket-native-build:$(sha256sum "$repo/scripts/container/Containerfile" | cut -c1-12)
if ! "$engine" image inspect "$image" >/dev/null 2>&1; then
    echo "== making the build container, once"
    "$engine" build -f "$repo/scripts/container/Containerfile" -t "$image" "$repo/scripts/container"
fi

mkdir -p "$out/.work"
user=(--user "$(id -u):$(id -g)")
[ "$(basename "$engine")" = podman ] && user=(--userns=keep-id)
for rom in "${roms[@]}"; do
    "$engine" run --rm "${user[@]}" \
        -v "$repo:/src:ro,z" -v "$out/.work:/work:z" -v "$out:/out:z" -v "$(dirname "$rom"):/rom:ro,z" \
        -e CARGO_HOME=/work/cargo -e CARGO_TARGET_DIR=/work/target -e HOME=/work \
        "$image" bash /src/scripts/container/inside-pc.sh "/rom/$(basename "$rom")"
done
echo "== done: run the .sh launchers in \"$out/Pocket Native (PC)\""
