# On a computer

Pocket Native plays in a window on Linux, with the keyboard and mouse, next
to the handhelds. The screens go into an X11 window (XWayland is fine), the
buttons come from the keyboard and the touch screen from the mouse pointer.

## Build

The same container builds it, the game included:

```bash
scripts/build-pc.sh /path/to/game.3ds
```

It leaves `build-pc/Pocket Native (PC)` with a launcher per game. Run the
launcher, or `pocket-native/run.sh <title id>`, and the game opens in a window.

## Controls

| Key | 3DS |
|---|---|
| W / A / S / D | Circle pad |
| Arrow keys | D-pad |
| I / J / K / L | Right stick (touch cursor, or the C-stick) |
| X / Z / C / V | A / B / X / Y |
| Q / E | L / R |
| 1 / 3 | ZL / ZR |
| 2 / 4 | L3 / R3 |
| Return | Start |
| Backspace | Select |
| F1 | FN: opens the menu, and with Start quits |
| Mouse, left button | Touch on the bottom screen |
| Escape, or the window's close button | Quits |

## Window

`ZAKURO_WINDOW_SIZE=1024x768` picks the window's size, in pixels. The screens
keep their shape in it and are centred, so any window size works.

## Two panels, one screen on each

A handheld with two independent panels, as the Anbernic RG DS Plus has,
gets the top screen on one and the bottom screen on the other, each fitted
to its own 1024x768 panel. The engine finds both connectors through KMS,
gives each one its own CRTC and draws the two screens apart: the top screen
400x240 sits 1024x614 in the middle of the upper panel, the bottom screen
follows the panels' 4:3 shape and fills the lower one. The menu goes on the
upper panel and the touch cursor follows the fingers on the lower one.

On a system whose desktop is Weston, as the RG DS Plus's is, the launcher
stops the compositor while a game runs and starts it again after, since the
game drives the panels itself.
