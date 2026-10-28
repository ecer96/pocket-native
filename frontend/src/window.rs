//! a computer's desktop instead of a handheld: an X11 window the screens go
//! into, its keyboard for the buttons, its mouse for the touch screen. The
//! X11 and the EGL surface on the window come through dlopen, as the rest of
//! the handheld's libraries do.
//!
//! ZAKURO_WINDOW_SIZE=1024x768 picks the window's size.

use std::ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CString};
use std::sync::{Arc, Mutex};

use crate::input::{self, Pad, A, B, DOWN, FN, L1, L2, L3, LEFT, R1, R2, R3, RIGHT, SELECT, START, STICK, UP, X, Y};

type Display = c_void;
type Window = c_ulong;
type Atom = c_ulong;

/// an XEvent is a union of 24 longs.
const XEVENT_LONGS: usize = 24;

const KEY_PRESS_MASK: c_long = 1 << 0;
const KEY_RELEASE_MASK: c_long = 1 << 1;
const BUTTON_PRESS_MASK: c_long = 1 << 2;
const BUTTON_RELEASE_MASK: c_long = 1 << 3;
const POINTER_MOTION_MASK: c_long = 1 << 6;
const EXPOSURE_MASK: c_long = 1 << 15;
const STRUCTURE_NOTIFY_MASK: c_long = 1 << 17;

const KEY_PRESS: c_int = 2;
const KEY_RELEASE: c_int = 3;
const BUTTON_PRESS: c_int = 4;
const BUTTON_RELEASE: c_int = 5;
const MOTION_NOTIFY: c_int = 6;
const CONFIGURE_NOTIFY: c_int = 22;
const CLIENT_MESSAGE: c_int = 33;

const XK_BACK_SPACE: c_ulong = 0xff08;
const XK_RETURN: c_ulong = 0xff0d;
const XK_ESCAPE: c_ulong = 0xff1b;
const XK_LEFT: c_ulong = 0xff51;
const XK_UP: c_ulong = 0xff52;
const XK_RIGHT: c_ulong = 0xff53;
const XK_DOWN: c_ulong = 0xff54;
const XK_F1: c_ulong = 0xffbe;

#[repr(C)]
#[derive(Clone, Copy)]
/// what key, button and motion events have in common, which is all of them
/// up to their last field.
struct XEventFields {
    kind: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    root: Window,
    subwindow: Window,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    detail: c_uint,
    same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XConfigureEvent {
    kind: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    event: Window,
    window: Window,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border_width: c_int,
    above: Window,
    override_redirect: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct XClientMessageEvent {
    kind: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut Display,
    window: Window,
    message_type: Atom,
    format: c_int,
    data: [c_long; 5],
}

type OpenDisplay = extern "C" fn(*const c_char) -> *mut Display;
type DefaultScreen = extern "C" fn(*mut Display) -> c_int;
type RootWindow = extern "C" fn(*mut Display, c_int) -> Window;
type CreateSimpleWindow =
    extern "C" fn(*mut Display, Window, c_int, c_int, c_uint, c_uint, c_uint, c_ulong, c_ulong) -> Window;
type StoreName = extern "C" fn(*mut Display, Window, *const c_char) -> c_int;
type SelectInput = extern "C" fn(*mut Display, Window, c_long) -> c_int;
type MapWindow = extern "C" fn(*mut Display, Window) -> c_int;
type Flush = extern "C" fn(*mut Display) -> c_int;
type Pending = extern "C" fn(*mut Display) -> c_int;
type NextEvent = extern "C" fn(*mut Display, *mut c_void) -> c_int;
type InternAtom = extern "C" fn(*mut Display, *const c_char, c_int) -> Atom;
type SetWMProtocols = extern "C" fn(*mut Display, Window, *mut Atom, c_int) -> c_int;
type CloseDisplay = extern "C" fn(*mut Display) -> c_int;
type LookupKeysym = extern "C" fn(*mut c_void, c_int) -> c_ulong;
type AutoRepeatOff = extern "C" fn(*mut Display) -> c_int;

macro_rules! symbol {
    ($library:expr, $name:literal, $type:ty) => {{
        // SAFETY: the symbol is the one named, read as the signature Xlib
        // has for it
        let found = $library.get::<$type>(concat!($name, "\0").as_bytes());
        *found.ok()?
    }};
}

/// the Xlib calls, plain function pointers that copy to the window's thread.
#[derive(Clone, Copy)]
struct Xlib {
    open_display: OpenDisplay,
    default_screen: DefaultScreen,
    root_window: RootWindow,
    create_simple_window: CreateSimpleWindow,
    store_name: StoreName,
    select_input: SelectInput,
    map_window: MapWindow,
    flush: Flush,
    pending: Pending,
    next_event: NextEvent,
    intern_atom: InternAtom,
    set_wm_protocols: SetWMProtocols,
    close_display: CloseDisplay,
    lookup_keysym: LookupKeysym,
    auto_repeat_off: AutoRepeatOff,
}

impl Xlib {
    fn load() -> Option<(Xlib, libloading::Library)> {
        // SAFETY: plain dlopen of the system's libX11, every symbol read as
        // the signature Xlib has for it
        unsafe {
            let library = libloading::Library::new("libX11.so.6").or_else(|_| libloading::Library::new("libX11.so")).ok()?;
            let xlib = Xlib {
                open_display: symbol!(library, "XOpenDisplay", OpenDisplay),
                default_screen: symbol!(library, "XDefaultScreen", DefaultScreen),
                root_window: symbol!(library, "XRootWindow", RootWindow),
                create_simple_window: symbol!(library, "XCreateSimpleWindow", CreateSimpleWindow),
                store_name: symbol!(library, "XStoreName", StoreName),
                select_input: symbol!(library, "XSelectInput", SelectInput),
                map_window: symbol!(library, "XMapWindow", MapWindow),
                flush: symbol!(library, "XFlush", Flush),
                pending: symbol!(library, "XPending", Pending),
                next_event: symbol!(library, "XNextEvent", NextEvent),
                intern_atom: symbol!(library, "XInternAtom", InternAtom),
                set_wm_protocols: symbol!(library, "XSetWMProtocols", SetWMProtocols),
                close_display: symbol!(library, "XCloseDisplay", CloseDisplay),
                lookup_keysym: symbol!(library, "XLookupKeysym", LookupKeysym),
                auto_repeat_off: symbol!(library, "XAutoRepeatOff", AutoRepeatOff),
            };
            Some((xlib, library))
        }
    }
}

/// what the window thread keeps between events.
struct Held {
    keys: Vec<u32>,
    mouse: Option<(f32, f32)>,
    window: (u32, u32),
    layout: (u32, u32),
}

impl Held {
    /// the pad as the keys and the mouse have it.
    fn fill(&self, pad: &mut Pad) {
        pad.keys.clear();
        let (mut across, mut down) = (0.0f32, 0.0f32);
        let (mut look_across, mut look_down) = (0.0f32, 0.0f32);
        for key in &self.keys {
            match *key {
                k if k == 'w' as u32 => down -= 1.0,
                k if k == 's' as u32 => down += 1.0,
                k if k == 'a' as u32 => across -= 1.0,
                k if k == 'd' as u32 => across += 1.0,
                k if k == 'i' as u32 => look_down -= 1.0,
                k if k == 'k' as u32 => look_down += 1.0,
                k if k == 'j' as u32 => look_across -= 1.0,
                k if k == 'l' as u32 => look_across += 1.0,
                k => {
                    if let Some(code) = pad_key(k as u16) {
                        pad.keys.insert(code);
                    }
                }
            }
        }
        pad.axes[0] = (across * STICK) as i32;
        pad.axes[1] = (down * STICK) as i32;
        pad.axes[3] = (look_across * STICK) as i32;
        pad.axes[4] = (look_down * STICK) as i32;
        match self.mouse {
            Some((x, y)) => {
                let (across, down) = self.to_layout(x, y);
                let finger = (0, across, down);
                if pad.fingers.first().copied() != Some(finger) {
                    pad.fingers.clear();
                    pad.fingers.push(finger);
                }
            }
            None => {
                if let Some(finger) = pad.fingers.first().copied() {
                    pad.fingers.clear();
                    pad.lifted.push(finger);
                }
            }
        }
    }

    /// a point in the window as a point in the layout, which is centred in
    /// the window and kept in shape.
    fn to_layout(&self, x: f32, y: f32) -> (f32, f32) {
        let (width, height) = (self.window.0 as f32, self.window.1 as f32);
        let (layout_width, layout_height) = (self.layout.0 as f32, self.layout.1 as f32);
        let scale = (width / layout_width).min(height / layout_height).max(0.0001);
        let left = (width - layout_width * scale) / 2.0;
        let top = (height - layout_height * scale) / 2.0;
        (((x - left) / scale).clamp(0.0, layout_width), ((y - top) / scale).clamp(0.0, layout_height))
    }
}

/// a keyboard key as the button it stands for, if it is one.
fn pad_key(key: u16) -> Option<u16> {
    Some(match key {
        k if k == XK_LEFT as u16 => LEFT,
        k if k == XK_RIGHT as u16 => RIGHT,
        k if k == XK_UP as u16 => UP,
        k if k == XK_DOWN as u16 => DOWN,
        k if k == 'x' as u16 => A,
        k if k == 'z' as u16 => B,
        k if k == 'c' as u16 => X,
        k if k == 'v' as u16 => Y,
        k if k == 'q' as u16 => L1,
        k if k == 'e' as u16 => R1,
        k if k == '1' as u16 => L2,
        k if k == '3' as u16 => R2,
        k if k == XK_RETURN as u16 => START,
        k if k == XK_BACK_SPACE as u16 => SELECT,
        k if k == XK_F1 as u16 => FN,
        k if k == '2' as u16 => L3,
        k if k == '4' as u16 => R3,
        _ => return None,
    })
}

/// the window on the desktop, opened before the game is loaded and kept for
/// as long as it plays.
pub struct Desktop {
    /// the library stays loaded, and the window up, while the game plays.
    _library: libloading::Library,
    _display: *mut Display,
    _window: Window,
}

impl Desktop {
    /// opens a window and starts reading its keyboard and mouse into the
    /// shared pad, or none when there is no display to open one on.
    pub fn open() -> Option<Desktop> {
        if std::env::var_os("ZAKURO_KMS").is_some() {
            return None;
        }
        let (xlib, library) = Xlib::load()?;
        // SAFETY: Xlib calls on the display opened here, on this thread
        let display = (xlib.open_display)(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let screen = (xlib.default_screen)(display);
        let root = (xlib.root_window)(display, screen);
        let (width, height) = window_size();
        let window = (xlib.create_simple_window)(display, root, 0, 0, width, height, 0, 0, 0);
        if window == 0 {
            (xlib.close_display)(display);
            return None;
        }
        let title = CString::new("Pocket Native").ok()?;
        (xlib.store_name)(display, window, title.as_ptr());
        (xlib.select_input)(
            display,
            window,
            KEY_PRESS_MASK
                | KEY_RELEASE_MASK
                | BUTTON_PRESS_MASK
                | BUTTON_RELEASE_MASK
                | POINTER_MOTION_MASK
                | EXPOSURE_MASK
                | STRUCTURE_NOTIFY_MASK,
        );
        // the window's close button ends the game
        let delete = CString::new("WM_DELETE_WINDOW").ok()?;
        let mut protocols = [(xlib.intern_atom)(display, delete.as_ptr(), 0)];
        let protocols_name = CString::new("WM_PROTOCOLS").ok()?;
        let protocols_atom = (xlib.intern_atom)(display, protocols_name.as_ptr(), 0);
        (xlib.set_wm_protocols)(display, window, protocols.as_mut_ptr(), 1);
        // key repeat sends a release and a press of its own, which would
        // read as the key let go and pressed again
        (xlib.auto_repeat_off)(display);
        (xlib.map_window)(display, window);
        (xlib.flush)(display);
        // the EGL surface and the screens go on this window
        zakuro_gpu::set_window(window as *mut c_void);
        read(display, window, xlib, protocols_atom, input::share());
        log::info!("the screens go in a {width}x{height} window");
        Some(Desktop { _library: library, _display: display, _window: window })
    }
}

/// reads the window's events for good, on a thread of its own.
fn read(display: *mut Display, _window: Window, xlib: Xlib, wm_protocols: Atom, pad: Arc<Mutex<Pad>>) {
    let address = display as usize;
    let spawned = std::thread::Builder::new().name("window".into()).spawn(move || {
        let display = address as *mut Display;
        let mut held = Held { keys: Vec::new(), mouse: None, window: window_size(), layout: zakuro_gpu::layout_size() };
        let mut event = [0u64; XEVENT_LONGS];
        while !input::QUIT.load(std::sync::atomic::Ordering::Relaxed) {
            while (xlib.pending)(display) > 0 {
                (xlib.next_event)(display, event.as_mut_ptr() as *mut c_void);
                // SAFETY: every event XNextEvent returned is one of these
                let fields = unsafe { *(event.as_ptr() as *const XEventFields) };
                match fields.kind {
                    KEY_PRESS | KEY_RELEASE => {
                        // SAFETY: a key event is one of these with the keycode
                        let keysym = (xlib.lookup_keysym)(event.as_mut_ptr() as *mut c_void, 0);
                        let key = lowercase(keysym);
                        if fields.kind == KEY_PRESS {
                            if key == XK_ESCAPE as u32 {
                                input::QUIT.store(true, std::sync::atomic::Ordering::Relaxed);
                                continue;
                            }
                            if !held.keys.contains(&key) {
                                held.keys.push(key);
                            }
                        } else {
                            held.keys.retain(|held| *held != key);
                        }
                        let mut pad = pad.lock().unwrap();
                        pad.pad_used = true;
                        held.fill(&mut pad);
                    }
                    BUTTON_PRESS | BUTTON_RELEASE => {
                        if fields.detail == 1 {
                            held.mouse = (fields.kind == BUTTON_PRESS).then_some((fields.x as f32, fields.y as f32));
                            held.layout = zakuro_gpu::layout_size();
                            held.fill(&mut pad.lock().unwrap());
                        }
                    }
                    MOTION_NOTIFY => {
                        if held.mouse.is_some() {
                            held.mouse = Some((fields.x as f32, fields.y as f32));
                            held.fill(&mut pad.lock().unwrap());
                        }
                    }
                    CONFIGURE_NOTIFY => {
                        // SAFETY: a configure event is one of these
                        let configure = unsafe { *(event.as_ptr() as *const XConfigureEvent) };
                        if configure.width > 0 && configure.height > 0 {
                            held.window = (configure.width as u32, configure.height as u32);
                        }
                    }
                    CLIENT_MESSAGE => {
                        // SAFETY: a client message is one of these
                        let message = unsafe { *(event.as_ptr() as *const XClientMessageEvent) };
                        if message.message_type == wm_protocols && message.data[0] == 0 {
                            input::QUIT.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    _ => {}
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });
    if spawned.is_err() {
        log::warn!("could not start the window's own thread");
    }
}

/// the letters come as their capital when shift is held.
fn lowercase(keysym: c_ulong) -> u32 {
    if (0x41..=0x5a).contains(&keysym) {
        (keysym + 0x20) as u32
    } else {
        keysym as u32
    }
}

fn window_size() -> (u32, u32) {
    std::env::var("ZAKURO_WINDOW_SIZE")
        .ok()
        .as_deref()
        .and_then(|size| {
            let (across, down) = size.split_once('x')?;
            Some((across.trim().parse().ok()?, down.trim().parse().ok()?))
        })
        .unwrap_or((1024, 768))
}
