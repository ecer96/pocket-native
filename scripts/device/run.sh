#!/bin/bash
# run.sh <title id> [rom]: plays a game Pocket Native was built with, from the
# launcher scripts/build.sh puts beside this folder, or from 3ds.sh with the
# ROM it was given. EmulationStation waits, stopped, while the game runs, and
# the game starts again when its menu's Reset asks for it.
dir="$(cd "$(dirname "$0")" && pwd)"

# on the RG DS Plus, a panfrost folder beside the game holds Mesa and the
# kernel's Panfrost driver, else GeminusOS's own in /usr/lib/panfrost and
# /lib/modules/panfrost: the GPU goes from ARM's driver to it while the
# game runs, Weston stopped and the panels driven through KMS, since ARM's
# driver ran the copies of the screens out of order with the draws. All of
# it goes back after, and to ARM's driver with Weston if Panfrost won't bind.
gpu=fde60000.gpu
panfrost=0
modules="$dir/panfrost" mesa="$dir/panfrost/lib"
if [ ! -d "$mesa" ]; then
    modules=/lib/modules/panfrost mesa=/usr/lib/panfrost
fi
menu=/mnt/vendor/ctrl/dmenu_ln
paused=/tmp/pocket-native-paused

# Weston up and taking clients before the launcher is started again, which
# otherwise fails at once and GeminusOS's loop ends with it
weston_ready() {
    local socket="${XDG_RUNTIME_DIR:-/var/run}/${WAYLAND_DISPLAY:-wayland-0}"
    for _ in $(seq 100); do
        if pgrep -x weston >/dev/null && [ -S "$socket" ]; then
            sleep 1
            return
        fi
        sleep 0.1
    done
}

# ARM's driver, Weston and GeminusOS's launcher back as they were. ARM's
# driver bound again numbers its node anew, which its library still looks
# for as /dev/mali0. The loops that start the launcher, held while the game
# ran, where they started it again and again with no Weston to show it, go
# on; with none left, loadapp.sh's own can't be run twice, its lock held for
# good by the mount.exfat it started, so its loop runs apart from this script
desktop_back() {
    echo $gpu > /sys/bus/platform/drivers/panfrost/unbind
    echo $gpu > /sys/bus/platform/drivers/mali/bind
    local node
    node=$(ls /dev/mali[0-9]* 2>/dev/null | grep -v '^/dev/mali0$' | tail -1)
    if [ -n "$node" ] && { [ ! -e /dev/mali0 ] || [ -L /dev/mali0 ]; }; then
        ln -sf "$(basename "$node")" /dev/mali0
    fi
    pgrep -x weston >/dev/null || /etc/init.d/S49weston start >/dev/null 2>&1
    weston_ready
    [ -f "$paused" ] && kill -CONT $(cat "$paused") 2>/dev/null
    rm -f "$paused"
    if [ -x "$menu" ] && ! pgrep -f "^/bin/bash $menu" >/dev/null && ! pgrep -f "do $menu" >/dev/null && ! pgrep -f "^/bin/bash /mnt/vendor/ctrl/loadapp.sh" >/dev/null; then
        setsid bash -c ". /etc/profile.d/myenv.sh; export SDL_VIDEO_DOUBLE_BUFFER=1 SDL_RENDER_VSYNC=1; while [ -f $menu ] && [ ! -f /tmp/stopAPP.ini ]; do $menu || true; done" </dev/null >/dev/null 2>&1 &
    fi
}

# run.sh --restore <pid>, started apart in a session of its own once the GPU
# is Panfrost's: this script puts the desktop back itself before it ends, as
# GeminusOS's loop starts the launcher again the moment it does, but a
# launcher that ran the game as its own child ends with Weston and can take
# the script with it, and then the desktop comes back from here once the
# script and the game are both gone
if [ "${1:-}" = --restore ]; then
    # gone or a zombie, as it stays while the loop that started it is held
    running() { [ -e "/proc/$1" ] && ! grep -q '^State:[[:space:]]*Z' "/proc/$1/status" 2>/dev/null; }
    while running "$2" || pgrep -x pocket-native >/dev/null; do
        sleep 0.5
    done
    [ -e "/tmp/pocket-native-back.$2" ] || desktop_back
    rm -f "/tmp/pocket-native-back.$2"
    exit 0
fi

# the launcher going, as it does when Weston stops, sends no hangup this far
trap '' HUP

game="$dir/games/$1"
rom="${2:-$game/game.3ds}"
data="$game/data"
mkdir -p "$data" "$dir/config"

if [ -d "$mesa" ] && [ -e /sys/bus/platform/drivers/mali/$gpu ] && [ -x /etc/init.d/S49weston ]; then
    for module in gpu_sched drm_shmem_helper panfrost; do
        grep -q "^$module " /proc/modules || insmod "$modules/${module/gpu_sched/gpu-sched}.ko"
    done
    if [ -d /sys/bus/platform/drivers/panfrost ]; then
        # the loops that start the launcher held, which with Weston gone
        # would start it again and again for nothing while the game runs
        # none this script runs under, though: GeminusOS's loop runs a game
        # as DraStic's, the launcher gone, and waits for it to end
        ancestors=" "
        for (( pid = $$; pid > 1; )); do
            ancestors+="$pid "
            pid=$(awk '/^PPid:/ { print $2 }' /proc/$pid/status 2>/dev/null)
            [ -n "$pid" ] || break
        done
        for pid in $(pgrep -f "^/bin/bash /mnt/vendor/ctrl/loadapp.sh"; pgrep -f "^/bin/bash $menu"; pgrep -f "do $menu"); do
            [[ "$ancestors" == *" $pid "* ]] || echo $pid
        done > "$paused"
        kill -STOP $(cat "$paused") 2>/dev/null
        /etc/init.d/S49weston stop >/dev/null 2>&1
        # its socket goes with it, so that a new one says the next is up
        for _ in $(seq 50); do pgrep -x weston >/dev/null || break; sleep 0.1; done
        rm -f "${XDG_RUNTIME_DIR:-/var/run}/${WAYLAND_DISPLAY:-wayland-0}"
        # ARM's driver let go of while a process still has it open, Weston
        # on its way out, is a kernel BUG and a hung handheld
        free=0
        for _ in $(seq 50); do
            if ! ls -l /proc/[0-9]*/fd 2>/dev/null | grep -q -- '-> /dev/mali'; then
                free=1
                break
            fi
            sleep 0.2
        done
        if [ "$free" = 1 ] && echo $gpu > /sys/bus/platform/drivers/mali/unbind; then
            if echo $gpu > /sys/bus/platform/drivers/panfrost/bind 2>/dev/null; then
                panfrost=1
                # powered on throughout: woken from a runtime suspend on this
                # kernel, a job of the GPU's hung and its recovery oopsed
                echo on > /sys/bus/platform/devices/$gpu/power/control
                setsid "$dir/run.sh" --restore $$ </dev/null >/dev/null 2>&1 &
            else
                echo $gpu > /sys/bus/platform/drivers/mali/bind
            fi
        fi
        if [ "$panfrost" = 0 ]; then
            desktop_back
        fi
    fi
fi

# a system whose desktop is Weston, as the RG DS Plus's GeminusOS is, splits
# the panels itself: the game is a Wayland client in SDL windows, one screen
# on each panel, and Weston keeps running. Otherwise the panels are driven
# through KMS.
sdl=0
if [ "$panfrost" = 1 ]; then
    :
elif [ -n "${WAYLAND_DISPLAY:-}" ] && [ -S "${XDG_RUNTIME_DIR:-/var/run}/${WAYLAND_DISPLAY}" ]; then
    sdl=1
elif [ -S /var/run/wayland-0 ]; then
    sdl=1
    export XDG_RUNTIME_DIR=/var/run WAYLAND_DISPLAY=wayland-0
fi
if [ "$sdl" = 1 ]; then
    # the top screen goes to the rightmost output, the upper panel of the RG DS
    # Plus, DSI-2 at 1024,0, the bottom one to the other. On ARM's driver the
    # GL work stays on the emulation's thread: with a thread of its own, the
    # top screen's copy caught the bottom screen drawn after it
    export SDL_VIDEODRIVER=wayland ZAKURO_SDL=1
    [ -x /etc/init.d/S49weston ] && export ZAKURO_SYNC_GPU=1
else
    export ZAKURO_KMS=1
fi
export ZAKURO_FAST=1 ZAKURO_DATA="$data" ZAKURO_RECOMPILED="$game/title.so" ZAKURO_SHADER_MANIFEST="$data/shaders.txt"
export POCKET_NATIVE_TITLES="$dir/titles" POCKET_NATIVE_POWER="$dir/power.sh" POCKET_NATIVE_CONFIG="$dir/config"
# the compiled shaders beside the games, whatever HOME a launcher gives, which
# on GeminusOS is /, so one run's programs are the next one's
export XDG_CACHE_HOME="$dir/cache"
if [ "$panfrost" = 1 ]; then
    # Mesa's programs apart from ARM's, compiled on three cores at once, and
    # the uniform rings mapped for good, each quarter fenced
    export LD_LIBRARY_PATH="$mesa${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" XDG_CACHE_HOME="$dir/cache-panfrost" ZAKURO_COMPILER_THREADS=3 ZAKURO_PERSISTENT=1
fi
# glibc's arena a thread, each keeping what its thread once took, ran 1 GB
# with no swap out of memory a level into a game: two for all
export MALLOC_ARENA_MAX=2
# GeminusOS's speaker is the amplifier on the second card, not the codec on
# the first, which is the system's default
if [ -z "${ZAKURO_ALSA_DEVICE:-}" ] && [ -x /etc/init.d/S49weston ] && [ -d /proc/asound/card1 ]; then
    export ZAKURO_ALSA_DEVICE=plughw:1,0
fi
# draws to leave out besides the game's options, as ZAKURO_DROP_DRAWS takes them
[ -f "$data/drops.txt" ] && export ZAKURO_DROP_DRAWS="$(cat "$data/drops.txt")"

es=$(pgrep -f "^/usr/bin/emulationstation/emulationstation( |$)" || true)
[ -n "$es" ] && kill -STOP $es
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
# the desktop back before this script ends, which GeminusOS's loop takes as
# the moment to start the launcher again
if [ "$panfrost" = 1 ]; then
    desktop_back
    touch "/tmp/pocket-native-back.$$"
fi
"$dir/power.sh" balanced
[ -n "$es" ] && kill -CONT $es
sudo -n systemctl restart oga_events >/dev/null 2>&1 &
exit 0
