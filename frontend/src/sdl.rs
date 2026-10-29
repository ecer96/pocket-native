//! SDL windows for a system whose desktop is Wayland, as the RG DS Plus's
//! GeminusOS is: Weston owns the panels and splits them, so the game runs as
//! a Wayland client instead of driving KMS itself.
//!
//! One window goes on each panel, the top screen in the upper one and the
//! bottom screen in the lower one. SDL makes the windows and their OpenGL ES
//! contexts, sharing their objects, and the EGL handles behind them are
//! handed to the renderer, which draws and swaps on them. The buttons and the
//! touch panel still come from evdev.
//!
//! ZAKURO_SDL_DISPLAY picks the panel the top screen goes to, by SDL's
//! number or by the output's name, the rightmost output by default, the upper
//! panel on the RG DS Plus.

use std::ffi::{c_char, c_int, c_ulong, c_void, CStr, CString};
use std::sync::atomic::Ordering;

use crate::input;

type Window = *mut c_void;
type GlContext = *mut c_void;

const SDL_INIT_VIDEO: u32 = 0x0000_0020;

/// the window takes the display's size, which Weston's desktop shell takes
/// better than asking for fullscreen.
const SDL_WINDOW_FULLSCREEN_DESKTOP: u32 = 0x0000_1001;
const SDL_WINDOW_OPENGL: u32 = 0x0000_0002;
const SDL_WINDOW_SHOWN: u32 = 0x0000_0004;

const SDL_WINDOWPOS_CENTERED_DISPLAY: c_int = 0x2FFF_0000;

const SDL_GL_RED_SIZE: c_int = 0;
const SDL_GL_GREEN_SIZE: c_int = 1;
const SDL_GL_BLUE_SIZE: c_int = 2;
const SDL_GL_ALPHA_SIZE: c_int = 3;
const SDL_GL_DOUBLEBUFFER: c_int = 5;
const SDL_GL_DEPTH_SIZE: c_int = 12;
const SDL_GL_CONTEXT_MAJOR_VERSION: c_int = 17;
const SDL_GL_CONTEXT_MINOR_VERSION: c_int = 18;
const SDL_GL_CONTEXT_PROFILE_MASK: c_int = 21;
const SDL_GL_CONTEXT_PROFILE_ES: c_int = 0x0004;

const SDL_QUIT: u32 = 0x100;

const EGL_CONFIG_ID: c_ulong = 0x3028;
const EGL_DRAW: c_ulong = 0x3059;
const EGL_WIDTH: c_ulong = 0x3057;
const EGL_HEIGHT: c_ulong = 0x3056;
const EGL_NONE: c_ulong = 0x3038;
const EGL_NO_CONTEXT: *mut c_void = std::ptr::null_mut();

#[repr(C)]
struct Rect {
    x: c_int,
    y: c_int,
    w: c_int,
    h: c_int,
}

/// an SDL_Event is a union of 56 bytes.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
struct Event([u8; 64]);

impl Event {
    fn kind(&self) -> u32 {
        u32::from_ne_bytes([self.0[0], self.0[1], self.0[2], self.0[3]])
    }
}

macro_rules! symbol {
    ($library:expr, $name:literal, $type:ty) => {{
        // SAFETY: the symbol is the one named, read as its signature
        $library.get::<$type>(concat!($name, "\0").as_bytes()).ok().map(|found| *found)
    }};
}

/// SDL's entry points, plain function pointers.
#[derive(Clone, Copy)]
struct Sdl2 {
    init: extern "C" fn(u32) -> c_int,
    get_error: extern "C" fn() -> *const c_char,
    get_num_video_displays: extern "C" fn() -> c_int,
    get_display_bounds: extern "C" fn(c_int, *mut Rect) -> c_int,
    get_display_name: extern "C" fn(c_int) -> *const c_char,
    create_window: extern "C" fn(*const c_char, c_int, c_int, c_int, c_int, u32) -> Window,
    get_window_size: extern "C" fn(Window, *mut c_int, *mut c_int),
    get_window_position: extern "C" fn(Window, *mut c_int, *mut c_int),
    gl_set_attribute: extern "C" fn(c_int, c_int) -> c_int,
    gl_create_context: extern "C" fn(Window) -> GlContext,
    gl_make_current: extern "C" fn(Window, GlContext) -> c_int,
    gl_set_swap_interval: extern "C" fn(c_int) -> c_int,
    gl_swap_window: extern "C" fn(Window) -> c_int,
    poll_event: extern "C" fn(*mut Event) -> c_int,
    pump_events: extern "C" fn(),
    delay: extern "C" fn(u32),
}

impl Sdl2 {
    fn load() -> Option<(Sdl2, libloading::Library)> {
        // SAFETY: plain dlopen of the system's SDL2, every symbol read as the
        // signature SDL has for it
        unsafe {
            let library = libloading::Library::new("libSDL2-2.0.so.0").or_else(|_| libloading::Library::new("libSDL2.so")).ok()?;
            let sdl = Sdl2 {
                init: symbol!(library, "SDL_Init", extern "C" fn(u32) -> c_int)?,
                get_error: symbol!(library, "SDL_GetError", extern "C" fn() -> *const c_char)?,
                get_num_video_displays: symbol!(library, "SDL_GetNumVideoDisplays", extern "C" fn() -> c_int)?,
                get_display_bounds: symbol!(library, "SDL_GetDisplayBounds", extern "C" fn(c_int, *mut Rect) -> c_int)?,
                get_display_name: symbol!(library, "SDL_GetDisplayName", extern "C" fn(c_int) -> *const c_char)?,
                create_window: symbol!(
                    library,
                    "SDL_CreateWindow",
                    extern "C" fn(*const c_char, c_int, c_int, c_int, c_int, u32) -> Window
                )?,
                get_window_size: symbol!(library, "SDL_GetWindowSize", extern "C" fn(Window, *mut c_int, *mut c_int) -> ())?,
                get_window_position: symbol!(library, "SDL_GetWindowPosition", extern "C" fn(Window, *mut c_int, *mut c_int) -> ())?,
                gl_set_attribute: symbol!(library, "SDL_GL_SetAttribute", extern "C" fn(c_int, c_int) -> c_int)?,
                gl_create_context: symbol!(library, "SDL_GL_CreateContext", extern "C" fn(Window) -> GlContext)?,
                gl_make_current: symbol!(library, "SDL_GL_MakeCurrent", extern "C" fn(Window, GlContext) -> c_int)?,
                gl_set_swap_interval: symbol!(library, "SDL_GL_SetSwapInterval", extern "C" fn(c_int) -> c_int)?,
                gl_swap_window: symbol!(library, "SDL_GL_SwapWindow", extern "C" fn(Window) -> c_int)?,
                poll_event: symbol!(library, "SDL_PollEvent", extern "C" fn(*mut Event) -> c_int)?,
                pump_events: symbol!(library, "SDL_PumpEvents", extern "C" fn())?,
                delay: symbol!(library, "SDL_Delay", extern "C" fn(u32))?,
            };
            Some((sdl, library))
        }
    }
}

/// EGL's own calls, to take the contexts SDL made and read the surfaces it
/// draws on.
struct EglCalls {
    get_current_display: extern "C" fn() -> *mut c_void,
    get_current_context: extern "C" fn() -> *mut c_void,
    get_current_surface: extern "C" fn(c_ulong) -> *mut c_void,
    query_context: extern "C" fn(*mut c_void, *mut c_void, c_int, *mut c_int) -> c_int,
    choose_config: extern "C" fn(*mut c_void, *const c_int, *mut *mut c_void, c_int, *mut c_int) -> c_int,
    query_surface: extern "C" fn(*mut c_void, *mut c_void, c_int, *mut c_int) -> c_int,
}

impl EglCalls {
    fn load(library: &libloading::Library) -> Option<EglCalls> {
        // SAFETY: the symbols are EGL's, read as EGL has them
        unsafe {
            Some(EglCalls {
                get_current_display: symbol!(library, "eglGetCurrentDisplay", extern "C" fn() -> *mut c_void)?,
                get_current_context: symbol!(library, "eglGetCurrentContext", extern "C" fn() -> *mut c_void)?,
                get_current_surface: symbol!(library, "eglGetCurrentSurface", extern "C" fn(c_ulong) -> *mut c_void)?,
                query_context: symbol!(
                    library,
                    "eglQueryContext",
                    extern "C" fn(*mut c_void, *mut c_void, c_int, *mut c_int) -> c_int
                )?,
                choose_config: symbol!(
                    library,
                    "eglChooseConfig",
                    extern "C" fn(*mut c_void, *const c_int, *mut *mut c_void, c_int, *mut c_int) -> c_int
                )?,
                query_surface: symbol!(
                    library,
                    "eglQuerySurface",
                    extern "C" fn(*mut c_void, *mut c_void, c_int, *mut c_int) -> c_int
                )?,
            })
        }
    }
}

/// what one window's context and surface are, as EGL sees them.
struct Made {
    display: *mut c_void,
    config: *mut c_void,
    context: *mut c_void,
    surface: *mut c_void,
    size: (c_int, c_int),
}

/// the windows the game plays in, kept for as long as it plays.
pub struct Sdl {
    _library: libloading::Library,
    _egl_library: libloading::Library,
    _upper: Window,
    _lower: Window,
    _context: GlContext,
}

impl Sdl {
    /// opens a fullscreen window on each panel with an OpenGL ES 3.1 context,
    /// sharing their objects, and hands the EGL side to the renderer, or none
    /// when SDL or its video driver are not there.
    pub fn open() -> Option<Sdl> {
        let (sdl, library) = Sdl2::load()?;
        // SAFETY: plain SDL calls, with the library kept for as long as the
        // windows live
        unsafe {
            if (sdl.init)(SDL_INIT_VIDEO) != 0 {
                log::warn!("no SDL video, {}", CStr::from_ptr((sdl.get_error)()).to_string_lossy());
                return None;
            }
            let displays = (sdl.get_num_video_displays)();
            if displays <= 0 {
                log::warn!("SDL has no displays");
                return None;
            }
            // the displays by the names Weston gives its outputs, which stay
            // put where SDL's numbering follows the order they were announced
            let names: Vec<String> = (0..displays)
                .map(|index| {
                    let name = (sdl.get_display_name)(index);
                    if name.is_null() { String::new() } else { CStr::from_ptr(name).to_string_lossy().into_owned() }
                })
                .collect();
            log::info!("SDL displays: {names:?}");
            // ZAKURO_SDL_DISPLAY is the top screen's panel, by number or by
            // name; else the rightmost output, the upper panel of the RG DS
            // Plus, DSI-2, once its weston.ini places DSI-1, the lower one, at
            // 0,0 and DSI-2 at 1024,0. SDL's numbers follow the order Weston
            // announces the outputs in, which changes from one boot to the next
            let rightmost = (0..displays)
                .max_by_key(|&index| {
                    let mut bounds = Rect { x: 0, y: 0, w: 0, h: 0 };
                    (sdl.get_display_bounds)(index, &mut bounds);
                    (bounds.x, bounds.y)
                })
                .unwrap_or(0);
            let wanted = std::env::var("ZAKURO_SDL_DISPLAY").unwrap_or_default();
            let wanted = wanted.trim();
            let upper = wanted
                .parse::<c_int>()
                .ok()
                .filter(|index| *index >= 0 && *index < displays)
                .or_else(|| names.iter().position(|name| !wanted.is_empty() && name.contains(wanted)).map(|index| index as c_int))
                .unwrap_or(rightmost);
            let lower = if displays > 1 { if upper == 0 { 1 } else { 0 } } else { upper };
            for attribute in [
                (SDL_GL_RED_SIZE, 8),
                (SDL_GL_GREEN_SIZE, 8),
                (SDL_GL_BLUE_SIZE, 8),
                (SDL_GL_ALPHA_SIZE, 0),
                (SDL_GL_DEPTH_SIZE, 0),
                (SDL_GL_DOUBLEBUFFER, 1),
                (SDL_GL_CONTEXT_MAJOR_VERSION, 3),
                (SDL_GL_CONTEXT_MINOR_VERSION, 1),
                (SDL_GL_CONTEXT_PROFILE_MASK, SDL_GL_CONTEXT_PROFILE_ES),
            ] {
                (sdl.gl_set_attribute)(attribute.0, attribute.1);
            }
            let egl_library = libloading::Library::new("libEGL.so.1").or_else(|_| libloading::Library::new("libEGL.so")).ok()?;
            let egl = EglCalls::load(&egl_library)?;
            // the top screen's window, on the upper panel
            let upper_window = window_on(&sdl, upper, true)?;
            let upper_context = (sdl.gl_create_context)(upper_window);
            if upper_context.is_null() {
                log::warn!("no OpenGL ES context on the SDL window, {}", CStr::from_ptr((sdl.get_error)()).to_string_lossy());
                return None;
            }
            if (sdl.gl_make_current)(upper_window, upper_context) != 0 {
                log::warn!("could not make the SDL window's context current, {}", CStr::from_ptr((sdl.get_error)()).to_string_lossy());
                return None;
            }
            // the window paces itself; the game's own frame cap does the rest
            (sdl.gl_set_swap_interval)(0);
            let upper_made = made(&egl, &sdl, upper_window)?;
            // the bottom screen's window, on the lower panel, drawn with the
            // first window's context: both have the same config, and a context
            // of its own would not have the first one's vertex arrays and
            // framebuffers
            let mut lower_window = std::ptr::null_mut();
            if displays > 1 {
                if let Some(window) = window_on(&sdl, lower, true) {
                    if (sdl.gl_make_current)(window, upper_context) == 0 {
                        (sdl.gl_set_swap_interval)(0);
                        if let Some(lower_made) = made(&egl, &sdl, window) {
                            zakuro_gpu::adopt_second(lower_made.context, lower_made.surface, lower_made.size.0, lower_made.size.1);
                            log::info!("the bottom screen goes to panel {lower}, {}x{}", lower_made.size.0, lower_made.size.1);
                            lower_window = window;
                        }
                    } else {
                        log::warn!("could not draw on the second SDL window, {}", CStr::from_ptr((sdl.get_error)()).to_string_lossy());
                    }
                }
            }
            // the renderer's own thread makes the first context current
            (sdl.gl_make_current)(upper_window, EGL_NO_CONTEXT as GlContext);
            zakuro_gpu::adopt(upper_made.display, upper_made.config, upper_made.context, upper_made.surface, upper_made.size.0, upper_made.size.1);
            log::info!("the top screen goes to panel {upper}, {}x{}", upper_made.size.0, upper_made.size.1);
            // the renderer puts each picture up through SDL, which commits the
            // window's Wayland surface
            WINDOWS.lock().unwrap()[0] = upper_window as usize;
            WINDOWS.lock().unwrap()[1] = lower_window as usize;
            SWAP.store(sdl.gl_swap_window as usize, Ordering::Relaxed);
            ERROR.store(sdl.get_error as usize, Ordering::Relaxed);
            MAKE_CURRENT.store(sdl.gl_make_current as usize, Ordering::Relaxed);
            CONTEXTS.lock().unwrap()[0] = upper_context as usize;
            CONTEXTS.lock().unwrap()[1] = upper_context as usize;
            zakuro_gpu::set_swap(swap);
            pump(sdl);
            Some(Sdl { _library: library, _egl_library: egl_library, _upper: upper_window, _lower: lower_window, _context: upper_context })
        }
    }
}

/// the windows' SDL calls, for the renderer's thread to swap them with.
static SWAP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static WINDOWS: std::sync::Mutex<[usize; 2]> = std::sync::Mutex::new([0; 2]);
static ERROR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static MAKE_CURRENT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CONTEXTS: std::sync::Mutex<[usize; 2]> = std::sync::Mutex::new([0; 2]);

/// puts the picture the renderer drew up on that panel's window.
extern "C" fn swap(head: u32) {
    let swap = SWAP.load(Ordering::Relaxed);
    if swap == 0 {
        return;
    }
    let window = WINDOWS.lock().unwrap()[head as usize % 2];
    if window == 0 {
        return;
    }
    // SDL swaps a window only with its context current as SDL knows it, which
    // the renderer's thread made current through EGL instead
    let make_current = MAKE_CURRENT.load(Ordering::Relaxed);
    if make_current != 0 {
        let context = CONTEXTS.lock().unwrap()[head as usize % 2];
        // SAFETY: the address is SDL_GL_MakeCurrent, with the window this made
        // and the context SDL made on it
        let make_current: extern "C" fn(Window, GlContext) -> c_int = unsafe { std::mem::transmute(make_current) };
        make_current(window as Window, context as GlContext);
    }
    // SAFETY: the address is SDL_GL_SwapWindow, and the window is the one
    // this made and keeps
    let swap: extern "C" fn(Window) -> c_int = unsafe { std::mem::transmute(swap) };
    if swap(window as Window) != 0 {
        static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !SAID.swap(true, Ordering::Relaxed) {
            let error = ERROR.load(Ordering::Relaxed);
            // SAFETY: the address is SDL_GetError, which returns its own text
            let error: extern "C" fn() -> *const c_char = unsafe { std::mem::transmute(error) };
            log::warn!("could not swap the panel {head}'s window, {}", unsafe { CStr::from_ptr(error()) }.to_string_lossy());
        }
    }
}

/// a fullscreen window on this display, its size what the display has.
unsafe fn window_on(sdl: &Sdl2, display: c_int, opengl: bool) -> Option<Window> {
    let mut bounds = Rect { x: 0, y: 0, w: 1024, h: 768 };
    (sdl.get_display_bounds)(display, &mut bounds);
    let title = CString::new("Pocket Native").ok()?;
    let mut flags = SDL_WINDOW_SHOWN;
    if opengl {
        flags |= SDL_WINDOW_OPENGL;
    }
    // made fullscreen on the display asked for, which SDL takes from where
    // the window is put
    let window = (sdl.create_window)(
        title.as_ptr(),
        SDL_WINDOWPOS_CENTERED_DISPLAY + display,
        SDL_WINDOWPOS_CENTERED_DISPLAY + display,
        bounds.w,
        bounds.h,
        flags | SDL_WINDOW_FULLSCREEN_DESKTOP,
    );
    if window.is_null() {
        log::warn!("no SDL window on display {display}, {}", CStr::from_ptr((sdl.get_error)()).to_string_lossy());
        return None;
    }
    let (mut at_x, mut at_y, mut size_x, mut size_y) = (0, 0, 0, 0);
    (sdl.get_window_position)(window, &mut at_x, &mut at_y);
    (sdl.get_window_size)(window, &mut size_x, &mut size_y);
    log::info!("the window for display {display} is {size_x}x{size_y} at {at_x},{at_y}");
    Some(window)
}

/// what EGL made of a window SDL has made current: its display, config,
/// context, surface and size.
unsafe fn made(egl: &EglCalls, sdl: &Sdl2, window: Window) -> Option<Made> {
    let display = (egl.get_current_display)();
    let context = (egl.get_current_context)();
    let surface = (egl.get_current_surface)(EGL_DRAW);
    if display.is_null() || context.is_null() || surface.is_null() {
        log::warn!("the SDL window is not on EGL");
        return None;
    }
    let mut config_id: c_int = 0;
    (egl.query_context)(display, context, EGL_CONFIG_ID as c_int, &mut config_id);
    let attributes = [EGL_CONFIG_ID as c_int, config_id, EGL_NONE as c_int];
    let mut config: *mut c_void = std::ptr::null_mut();
    let mut found: c_int = 0;
    if (egl.choose_config)(display, attributes.as_ptr(), &mut config, 1, &mut found) == 0 || found <= 0 {
        log::warn!("could not find the EGL config SDL's window uses");
        return None;
    }
    let (mut width, mut height) = (0, 0);
    (egl.query_surface)(display, surface, EGL_WIDTH as c_int, &mut width);
    (egl.query_surface)(display, surface, EGL_HEIGHT as c_int, &mut height);
    if width <= 0 || height <= 0 {
        let (mut w, mut h) = (0, 0);
        (sdl.get_window_size)(window, &mut w, &mut h);
        width = w;
        height = h;
    }
    Some(Made { display, config, context, surface, size: (width.max(1), height.max(1)) })
}

/// keeps SDL's events moving, which a Wayland window needs to hear from the
/// compositor, and ends the game when the window is closed.
fn pump(sdl: Sdl2) {
    let spawned = std::thread::Builder::new().name("sdl".into()).spawn(move || {
        let mut event = Event([0; 64]);
        while !input::QUIT.load(Ordering::Relaxed) {
            (sdl.pump_events)();
            while (sdl.poll_event)(&mut event) > 0 {
                if event.kind() == SDL_QUIT {
                    input::QUIT.store(true, Ordering::Relaxed);
                }
            }
            (sdl.delay)(4);
        }
    });
    if spawned.is_err() {
        log::warn!("could not start SDL's own thread");
    }
}

