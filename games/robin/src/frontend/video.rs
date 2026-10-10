//! The window: the tape prompt when no tape was found (#70), then the
//! picture with its border, scaled by the GPU, a frame of the game every
//! 1/50.02 of a second, its sound, and the keyboard read, adapted from
//! starquake-recompiled's (`REUSED.md`).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Theme, Window, WindowId};

use robin::assets::Assets;
use robin::picture::{FULL_H, FULL_W};
use robin::session::{Session, State};

use super::FRAMES_PER_SECOND;
use super::audio::{Mixer, Output};
use super::gamepad::Gamepad;
use super::prompt::{self, Outcome, Prompt};
use super::text::Canvas;

/// The window's scale at first.
const SCALE: f64 = 3.0;

struct App {
    /// The game, once there's a tape.
    game: Option<(Assets, Session)>,
    /// The screen asking for the tape, while it's up.
    prompt: Option<Prompt>,
    /// Device pixels per logical pixel, which the prompt draws at.
    scale: f64,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    /// The host keys down, which the controls are built from. Cleared when
    /// the window stops listening, since some platforms send no key-ups
    /// then.
    held: HashSet<KeyCode>,
    gamepad: Gamepad,
    /// The sound card, if there is one, and the mixer making its samples.
    audio: Option<Output>,
    mixer: Mixer,
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
        self.scale = window.scale_factor();
        let (bw, bh) = self.buffer_size();
        match Pixels::new(bw, bh, surface) {
            Ok(mut p) => {
                if self.prompt.is_some() {
                    p.clear_color(clear_colour(prompt::BACKGROUND));
                }
                self.pixels = Some(p);
            }
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
        if self.prompt.is_some() {
            self.prompt_event(event_loop, event);
            return;
        }
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
                if let (Some(p), Some((_, session))) = (&mut self.pixels, &self.game) {
                    let picture = session.picture();
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
        let Some((assets, session)) = &mut self.game else {
            // The prompt changes only when something happens to it.
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };
        let now = Instant::now();
        if self.window.is_some() && now >= self.next {
            let mut controls = super::input::build(&self.held);
            let pad = self.gamepad.poll();
            // In play, the arrows and the pad go through the method the menu
            // chose; elsewhere they're the Kempston joystick, which the
            // menu doesn't read, so fire can't press a menu key (#84).
            let stick = super::input::stick(&self.held) | pad.kempston;
            if session.state == State::Playing {
                robin::movement::press(&session.game, &mut controls, stick);
            } else {
                controls.kempston |= stick;
            }
            if pad.start {
                controls.keys[4] &= !1;
            }
            session.frame(assets, controls);
            self.mixer.frame(
                session.io.ula_writes(),
                session.io.ay_writes.iter().copied(),
            );
            // Paced by the clock, at the 128K's frame rate. The card's clock
            // and this one drift apart slowly, so the period leans a little
            // when the card's queue strays outside two to three frames of
            // sound: enough to ride out a late wake-up, too little to hear
            // (starquake-recompiled).
            let mut period = self.period;
            if let Some(out) = &self.audio {
                out.push(self.mixer.samples());
                let frame = (f64::from(out.rate()) / FRAMES_PER_SECOND) as usize;
                let queued = out.queued();
                if queued < frame * 2 {
                    period = period.saturating_sub(Duration::from_micros(500));
                } else if queued > frame * 3 {
                    period += Duration::from_micros(500);
                }
            }
            self.mixer.clear_samples();
            self.next += period;
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
    /// The frame buffer's size: the Spectrum's picture with its border, or,
    /// for the prompt, the window at its real pixel density so the text is
    /// sharp.
    fn buffer_size(&self) -> (u32, u32) {
        if self.prompt.is_some() {
            (
                (f64::from(prompt::WIDTH) * self.scale).round() as u32,
                (f64::from(prompt::HEIGHT) * self.scale).round() as u32,
            )
        } else {
            (FULL_W as u32, FULL_H as u32)
        }
    }

    fn prompt_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        let Some(prompt) = &mut self.prompt else {
            return;
        };
        let outcome = match event {
            WindowEvent::CloseRequested => Outcome::Quit,
            WindowEvent::KeyboardInput { event, .. } => match event.physical_key {
                PhysicalKey::Code(code)
                    if event.state == ElementState::Pressed && !event.repeat =>
                {
                    prompt.key(code)
                }
                _ => Outcome::Nothing,
            },
            WindowEvent::CursorMoved { position, .. } => match &self.pixels {
                Some(p) => {
                    let (x, y) = p
                        .window_pos_to_pixel((position.x as f32, position.y as f32))
                        .unwrap_or_else(|(x, y)| (x.max(0) as usize, y.max(0) as usize));
                    let s = self.scale as f32;
                    prompt.cursor(x as f32 / s, y as f32 / s)
                }
                None => Outcome::Nothing,
            },
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => prompt.clicked(),
            WindowEvent::DroppedFile(path) => prompt.dropped(&path),
            WindowEvent::Resized(size) => {
                if let Some(p) = &mut self.pixels {
                    let _ = p.resize_surface(size.width, size.height);
                }
                Outcome::Redraw
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor;
                let (w, h) = self.buffer_size();
                if let Some(p) = &mut self.pixels {
                    let _ = p.resize_buffer(w, h);
                }
                Outcome::Redraw
            }
            WindowEvent::RedrawRequested => {
                let (w, h) = self.buffer_size();
                if let (Some(p), Some(prompt)) = (&mut self.pixels, &mut self.prompt) {
                    let mut canvas = Canvas {
                        pixels: p.frame_mut(),
                        width: w as usize,
                        height: h as usize,
                        scale: self.scale as f32,
                    };
                    prompt.draw(&mut canvas);
                    if let Err(e) = p.render() {
                        self.error = Some(e.to_string());
                        event_loop.exit();
                    }
                }
                Outcome::Nothing
            }
            _ => Outcome::Nothing,
        };
        match outcome {
            Outcome::Nothing => {}
            Outcome::Redraw => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            Outcome::Quit => event_loop.exit(),
            Outcome::Start(tape) => match robin::assets::read_tape(&tape.bytes) {
                Ok(assets) => {
                    let session = Session::new(&assets, seed());
                    self.game = Some((assets, session));
                    self.prompt = None;
                    let (w, h) = self.buffer_size();
                    if let Some(p) = &mut self.pixels {
                        let _ = p.resize_buffer(w, h);
                        p.clear_color(pixels::wgpu::Color::BLACK);
                    }
                    self.next = Instant::now();
                }
                Err(e) => {
                    self.error = Some(e);
                    event_loop.exit();
                }
            },
        }
    }

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

/// A seed for the game's random numbers, from the time.
fn seed() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
}

/// A colour as the linear value `wgpu` wants for clearing an sRGB surface.
fn clear_colour([r, g, b]: [u8; 3]) -> pixels::wgpu::Color {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    pixels::wgpu::Color {
        r: linear(r),
        g: linear(g),
        b: linear(b),
        a: 1.0,
    }
}

/// Plays the game in a window from `tape`, or, with none, first asks for it,
/// until the window is closed.
///
/// # Errors
///
/// If the tape can't be read, or the window or its drawing can't be made.
pub fn run(tape: Option<Vec<u8>>) -> Result<(), String> {
    let game = match tape {
        Some(bytes) => {
            let assets = robin::assets::read_tape(&bytes)?;
            let session = Session::new(&assets, seed());
            Some((assets, session))
        }
        None => None,
    };
    let prompt = game.is_none().then(Prompt::new);
    // Without a sound card the game plays silent. The stream is held until
    // the window closes.
    let (audio, _stream) = match Output::start() {
        Ok((out, stream)) => (Some(out), Some(stream)),
        Err(e) => {
            eprintln!("no sound: {e}");
            (None, None)
        }
    };
    let mixer = Mixer::new(audio.as_ref().map_or(48_000, Output::rate));
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = App {
        game,
        prompt,
        scale: 1.0,
        window: None,
        pixels: None,
        held: HashSet::new(),
        gamepad: Gamepad::new(),
        audio,
        mixer,
        next: Instant::now(),
        period: Duration::from_secs_f64(1.0 / FRAMES_PER_SECOND),
        error: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.error.map_or(Ok(()), Err)
}
