#!/bin/bash
# run.sh <title id> [rom]: plays a game Pocket Native was built with, from the
# launcher scripts/build.sh puts beside this folder, or from 3ds.sh with the
# ROM it was given. EmulationStation waits, stopped, while the game runs, and
# the game starts again when its menu's Reset asks for it.
dir="$(cd "$(dirname "$0")" && pwd)"
game="$dir/games/$1"
rom="${2:-$game/game.3ds}"
data="$game/data"
mkdir -p "$data" "$dir/config"

export ZAKURO_FAST=1 ZAKURO_KMS=1 ZAKURO_DATA="$data" ZAKURO_RECOMPILED="$game/title.so" ZAKURO_SHADER_MANIFEST="$data/shaders.txt"
export POCKET_NATIVE_TITLES="$dir/titles" POCKET_NATIVE_POWER="$dir/power.sh" POCKET_NATIVE_CONFIG="$dir/config"
# Super Mario 3D Land draws with the simplified fragment path on the Mali-G52
# of the RG DS Plus: the full combiner leaves its screen blue there and the
# simple one keeps it at the title's own rate. Other titles are untouched.
[ "$1" = "0004000000054000" ] && export ZAKURO_FS_DEBUG=simplemulti
# GeminusOS's speaker is the amplifier on the second card, not the codec on
# the first, which is the system's default
if [ -z "${ZAKURO_ALSA_DEVICE:-}" ] && [ -x /etc/init.d/S49weston ] && [ -d /proc/asound/card1 ]; then
    export ZAKURO_ALSA_DEVICE=plughw:1,0
fi
# draws to leave out besides the game's options, as ZAKURO_DROP_DRAWS takes them
[ -f "$data/drops.txt" ] && export ZAKURO_DROP_DRAWS="$(cat "$data/drops.txt")"

es=$(pgrep -f "^/usr/bin/emulationstation/emulationstation( |$)" || true)
[ -n "$es" ] && kill -STOP $es
# a system whose desktop is Weston, as the RG DS Plus's GeminusOS is, has the
# panels to itself: the screens are driven through KMS, so the compositor
# steps aside while a game runs and comes back after
weston=0
if [ -x /etc/init.d/S49weston ] && pgrep -x weston >/dev/null; then
    weston=1
    pkill -x weston 2>/dev/null || true
    sleep 0.5
fi
sudo -n sh -c "echo 1 > /proc/sys/debug/exception-trace" 2>/dev/null
cd "$game"
while true; do
    "$dir/pocket-native" "$rom" > "$data/log.txt" 2>&1
    status=$?
    [ $status -eq 3 ] || break
done
if [ $status -ne 0 ]; then
    echo "exit status $status" >> "$data/log.txt"
    dmesg | tail -40 >> "$data/log.txt"
fi
"$dir/power.sh" balanced
[ -n "$es" ] && kill -CONT $es
[ "$weston" = 1 ] && /etc/init.d/S49weston start >/dev/null 2>&1
sudo -n systemctl restart oga_events >/dev/null 2>&1 &
exit 0
