//! The window: the picture with its border, scaled by the GPU, a frame of
//! the game every 1/50.02 of a second, and the keyboard read, adapted from
//! starquake-recompiled's (`REUSED.md`).

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Theme, Window, WindowId};

use robin::assets::Assets;
use robin::picture::{FULL_H, FULL_W};
use robin::session::Session;

use super::FRAMES_PER_SECOND;

/// The window's scale at first.
const SCALE: f64 = 3.0;

struct App {
    assets: Assets,
    session: Session,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    /// The host keys down, which the controls are built from. Cleared when
    /// the window stops listening, since some platforms send no key-ups
    /// then.
    held: HashSet<KeyCode>,
    /// When the next frame is due.
    next: Instant,
    period: Duration,
    error: Option<String>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Robin of the Wood")
            // Dark, so the title bar doesn't read as a stripe over the
            // border.
            .with_theme(Some(Theme::Dark))
            .with_inner_size(LogicalSize::new(
                FULL_W as f64 * SCALE,
                FULL_H as f64 * SCALE,
            ))
            .with_min_inner_size(LogicalSize::new(FULL_W as f64, FULL_H as f64));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(e.to_string());
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        // Some compositors report 0x0 before the first configure.
        let surface = SurfaceTexture::new(size.width.max(1), size.height.max(1), window.clone());
        match Pixels::new(FULL_W as u32, FULL_H as u32, surface) {
            Ok(p) => self.pixels = Some(p),
            Err(e) => {
                self.error = Some(e.to_string());
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        self.next = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                if event.repeat {
                    return;
                }
                let pressed = event.state == ElementState::Pressed;
                if code == KeyCode::F11 {
                    if pressed {
                        self.toggle_fullscreen();
                    }
                } else if pressed {
                    self.held.insert(code);
                } else {
                    self.held.remove(&code);
                }
            }
            WindowEvent::Focused(false) => self.held.clear(),
            WindowEvent::Resized(size) => {
                if let Some(p) = &mut self.pixels {
                    let _ = p.resize_surface(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(p) = &mut self.pixels {
                    let picture = self.session.picture();
                    for (out, c) in p.frame_mut().as_chunks_mut::<4>().0.iter_mut().zip(picture) {
                        *out = [(c >> 16) as u8, (c >> 8) as u8, c as u8, 0xFF];
                    }
                    if let Err(e) = p.render() {
                        self.error = Some(e.to_string());
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if self.window.is_some() && now >= self.next {
            let controls = super::input::build(&self.held);
            self.session.frame(&self.assets, controls);
            self.next += self.period;
            // Behind by more than a frame (the machine was busy, or asleep):
            // carry on from now rather than rushing to catch up.
            if self.next < now {
                self.next = now + self.period;
            }
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
    }
}

impl App {
    fn toggle_fullscreen(&mut self) {
        if let Some(w) = &self.window {
            let to = match w.fullscreen() {
                Some(_) => None,
                None => Some(Fullscreen::Borderless(None)),
            };
            w.set_fullscreen(to);
        }
    }
}

/// Plays the game from the tape at `path` in a window, until it's closed.
///
/// # Errors
///
/// If the tape can't be read, or the window or its drawing can't be made.
pub fn run(path: &Path) -> Result<(), String> {
    let assets = robin::assets::read_game(path)?;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
    let session = Session::new(&assets, seed);
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = App {
        assets,
        session,
        window: None,
        pixels: None,
        held: HashSet::new(),
        next: Instant::now(),
        period: Duration::from_secs_f64(1.0 / FRAMES_PER_SECOND),
        error: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.error.map_or(Ok(()), Err)
}
