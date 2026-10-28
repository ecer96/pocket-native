#!/bin/bash
# inside-pc.sh <rom>: builds Pocket Native and the game for this computer, in
# the container. /src is the repository, /work the scratch space kept between
# builds and /out where "Pocket Native (PC)" is put together.
set -euo pipefail

rom="$1"
cd /src

id=$(python3 tools/title_id.py "$rom")
name=$(sed -n '/^name = "/{s/^name = "\(.*\)"$/\1/p;q}' "titles/$id/profile.toml" 2>/dev/null || true)
echo "== $id ${name:-(no profile yet)}"

echo "== building the recompiler and Pocket Native for this computer"
cargo build --release --locked -p recomp3ds --bin 3dsrecomp
cargo build --release --locked -p pocket-native

echo "== recompiling the game for this computer"
overrides=()
[ -d "titles/$id/overrides" ] && overrides=(--overrides "titles/$id/overrides")
RECOMP_CFLAGS=-DRECOMP_FASTMEM CC=gcc "$CARGO_TARGET_DIR/release/3dsrecomp" build "$rom" "/work/recomp/$id" "${overrides[@]}"

if [ ! -f /work/shared_font.bin ]; then
    echo "== making the system font"
    FONT_URL=https://raw.githubusercontent.com/azahar-emu/azahar/c9e1e946a0b5d30a58ae14dee83c1f203a49622d/externals/open_source_archives/include/shared_font.app.romfs.h
    curl -sSfL "$FONT_URL" -o /work/shared_font.app.romfs.h
    python3 tools/make_shared_font.py /work/shared_font.app.romfs.h /work/shared_font.bin
fi

echo "== putting Pocket Native (PC) together"
top="/out/Pocket Native (PC)"
pn="$top/pocket-native"
mkdir -p "$pn/games/$id/sysdata"
strip -o "$pn/pocket-native" "$CARGO_TARGET_DIR/release/pocket-native"
rm -rf "$pn/titles" && cp -r titles "$pn/titles"
cp scripts/device/run-pc.sh "$pn/run.sh"
cp LICENSE CREDITS.md assets/fonts/OFL.txt "$pn/"
cp "/work/recomp/$id/$id.so" "$pn/games/$id/title.so"
cp /work/shared_font.bin "$pn/games/$id/sysdata/shared_font.bin"
cp "$rom" "$pn/games/$id/game.3ds"
launcher="$top/$(printf %s "${name:-$id}" | sed 's/: / - /g; s/[\\/:*?"<>|]//g').sh"
printf '#!/bin/bash\nexec "$(dirname "$0")/pocket-native/run.sh" %s\n' "$id" > "$launcher"
chmod +x "$launcher" "$pn/run.sh" "$pn/pocket-native"
echo "== done: run \"$top/$(basename "$launcher")\""
