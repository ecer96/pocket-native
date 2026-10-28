#!/bin/bash
# run-pc.sh <title id>: plays a game Pocket Native was built with, in a window
# on this computer, with the keyboard and mouse.
dir="$(cd "$(dirname "$0")" && pwd)"
game="$dir/games/$1"
data="$game/data"
mkdir -p "$data" "$dir/config"

export ZAKURO_FAST=1 ZAKURO_DATA="$data" ZAKURO_RECOMPILED="$game/title.so" ZAKURO_SHADER_MANIFEST="$data/shaders.txt"
export POCKET_NATIVE_TITLES="$dir/titles" POCKET_NATIVE_CONFIG="$dir/config"
# draws to leave out besides the game's options, as ZAKURO_DROP_DRAWS takes them
[ -f "$data/drops.txt" ] && export ZAKURO_DROP_DRAWS="$(cat "$data/drops.txt")"

cd "$game"
status=0
while true; do
    "$dir/pocket-native" "$game/game.3ds" > "$data/log.txt" 2>&1
    status=$?
    [ $status -eq 3 ] || break
done
if [ $status -ne 0 ]; then
    echo "exit status $status" >> "$data/log.txt"
fi
exit $status
