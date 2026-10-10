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
use super::aids::{Aid, Aids};
use super::audio::{Mixer, Output};
use super::gamepad::Gamepad;
use super::journal::Journal;
use super::overlay::Overlay;
use super::panel::{self, Panel};
use super::prompt::{self, Outcome, Prompt};
use super::saves::{self, Slot};
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
    /// The aids asked for, the panel that shows them and the picker that
    /// sets them, laid over the window at its own resolution (#92, #95).
    aids: Aids,
    /// What the player has found this game, which the aids show (#96).
    journal: Journal,
    /// What the assists have set aside in the game, to put back (#100).
    set_aside: robin::assists::SetAside,
    overlay: Option<Overlay>,
    panel: Panel,
    /// What the overlay was last drawn from: the aids' and the journal's
    /// versions, the objective and the size. It's redrawn only when that
    /// changes.
    drawn: Option<Drawn>,
    /// When the next frame is due.
    next: Instant,
    period: Duration,
    error: Option<String>,
}

/// What the overlay is drawn from: the aids' and the journal's versions,
/// the objective, and the size.
type Drawn = (
    u64,
    u64,
    Option<robin::objective::Objective>,
    bool,
    (u32, u32),
);

/// The window's width in the Spectrum's pixels: the picture and the panel
/// beside it (#95).
const WINDOW_W: usize = FULL_W + panel::PANEL_W as usize;

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
                WINDOW_W as f64 * SCALE,
                FULL_H as f64 * SCALE,
            ))
            .with_min_inner_size(LogicalSize::new(WINDOW_W as f64, FULL_H as f64));
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
                self.overlay = Some(Overlay::new(&p.context().device, p.render_texture_format()));
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
            WindowEvent::CloseRequested => {
                // The game in play is saved on quit, to be offered next time
                // (#92, Decision 6).
                if self.aids.is_on(Aid::Saves) {
                    let _ = self.save(Slot::OnQuit);
                }
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                if event.repeat {
                    return;
                }
                let pressed = event.state == ElementState::Pressed;
                if pressed && self.picker_key(code) {
                    return;
                }
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
                    // The picture on the left; the overlay covers the rest.
                    let frame = p.frame_mut().as_chunks_mut::<4>().0;
                    for (row, from) in frame
                        .as_chunks_mut::<WINDOW_W>()
                        .0
                        .iter_mut()
                        .zip(picture.chunks(FULL_W))
                    {
                        for (out, &c) in row.iter_mut().zip(from) {
                            *out = [(c >> 16) as u8, (c >> 8) as u8, c as u8, 0xFF];
                        }
                        row[FULL_W..].fill([0, 0, 0, 0xFF]);
                    }
                    let clip = p.context().scaling_renderer.clip_rect();
                    let objective = (session.state == robin::session::State::Playing)
                        .then(|| robin::objective::Objective::of(&session.game));
                    let hermit_met = robin::places::hermit_met(&session.game);
                    let key = (
                        self.aids.version(),
                        self.journal.version(),
                        objective,
                        hermit_met,
                        (clip.2, clip.3),
                    );
                    if self.drawn != Some(key)
                        && let Some(overlay) = &mut self.overlay
                    {
                        let scale = clip.2 as f32 / panel::WINDOW_W;
                        let mut canvas = overlay.canvas(clip.2, clip.3, scale);
                        let view = panel::View {
                            journal: Some(&self.journal),
                            objective,
                            hermit_met,
                            trade: robin::places::trade(&session.game),
                            doorways: robin::places::doorways(&session.game),
                        };
                        self.panel.draw(&mut canvas, &self.aids, &view);
                        self.drawn = Some(key);
                    }
                    let overlay = &mut self.overlay;
                    let rendered = p.render_with(|encoder, target, context| {
                        context.scaling_renderer.render(encoder, target);
                        if let Some(overlay) = overlay {
                            let clip = context.scaling_renderer.clip_rect();
                            overlay.render(&context.device, &context.queue, encoder, target, clip);
                        }
                        Ok(())
                    });
                    if let Err(e) = rendered {
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
        // The game stands still while the picker or the whole map is open
        // (#92, Decisions 7 and 9).
        if self.aids.paused() {
            self.next = now + self.period;
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next));
            return;
        }
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
            // The assists, around the game rather than in it (#100): health
            // and lives only in play; the witch's doorways put back
            // whenever no witch is off.
            let playing = session.state == State::Playing;
            let on = robin::assists::Assists {
                energy: playing && self.aids.is_on(Aid::Energy),
                lives: playing && self.aids.is_on(Aid::Lives),
                no_witch: self.aids.is_on(Aid::NoWitch),
            };
            robin::assists::apply(&mut session.game, on, &mut self.set_aside);
            self.journal.note(session);
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
            (WINDOW_W as u32, FULL_H as u32)
        }
    }

    /// The picker's keys (#95): F1 or Tab opens and closes it, neither a
    /// Spectrum key; while it's open, the arrows choose and Enter switches,
    /// and it has the keyboard to itself. ` or F2 opens and closes the whole
    /// map (#96); M, as the mockup had it, is Robin's "right". Whether the
    /// key was theirs.
    fn picker_key(&mut self, code: KeyCode) -> bool {
        // The offer of the game saved on quit takes the next key: Enter to
        // continue it, anything else to start afresh (#101).
        if self.aids.offer_open() {
            self.aids.set_offer(false);
            if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
                self.restore(Slot::OnQuit);
            } else {
                saves::remove(Slot::OnQuit);
            }
            self.held.clear();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }
        let open = self.aids.picker_open();
        if !open && !self.aids.paused() && self.aids.is_on(Aid::Saves) {
            match code {
                KeyCode::F5 => {
                    let notice = match self.save(Slot::Quick) {
                        Ok(true) => "Saved. F9 restores it.".to_string(),
                        Ok(false) => "Not now: saves are made in play, between sounds.".to_string(),
                        Err(e) => format!("Couldn't save: {e}"),
                    };
                    self.aids.set_notice(notice);
                    return true;
                }
                KeyCode::F9 => {
                    self.restore(Slot::Quick);
                    return true;
                }
                _ => {}
            }
        }
        if !open && matches!(code, KeyCode::Backquote | KeyCode::F2) && self.aids.is_on(Aid::Map) {
            self.aids.open_or_close_map();
            self.held.clear();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            return true;
        }
        if self.aids.full_map_open() && !matches!(code, KeyCode::F1 | KeyCode::Tab) {
            // The whole map has the keyboard while it's open.
            return true;
        }
        match code {
            KeyCode::F1 | KeyCode::Tab => {
                self.aids.open_or_close();
                // What was held is let go, since the game won't see the
                // key-ups while the picker has the keyboard.
                self.held.clear();
            }
            _ if !open => return false,
            KeyCode::ArrowUp => self.aids.move_focus(false),
            KeyCode::ArrowDown => self.aids.move_focus(true),
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => self.aids.toggle_focus(),
            _ => {}
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
        true
    }

    /// Saves the game in play to `slot`, with what the assists set aside put
    /// back first, so a save holds the game as it was (#100, #101). Whether
    /// there was a game to save.
    fn save(&mut self, slot: Slot) -> Result<bool, String> {
        let Some((_, session)) = &self.game else {
            return Ok(false);
        };
        let mut copy = session.clone();
        robin::assists::apply(
            &mut copy.game,
            robin::assists::Assists::default(),
            &mut self.set_aside.clone(),
        );
        let Some(saved) = copy.saved() else {
            return Ok(false);
        };
        saves::write(slot, &saved, &self.journal).map(|()| true)
    }

    /// Restores the game in `slot`, if there's one.
    fn restore(&mut self, slot: Slot) {
        let Some((saved, journal)) = saves::read(slot) else {
            self.aids.set_notice("Nothing saved yet: F5 saves.");
            return;
        };
        if let Some((_, session)) = &mut self.game {
            *session = Session::resume(&saved);
            self.journal = journal;
            self.set_aside = robin::assists::SetAside::default();
            self.aids.set_notice(match slot {
                Slot::Quick => "Restored. F5 saves again.",
                Slot::OnQuit => "Restored the game you quit.",
            });
            self.next = Instant::now();
        }
    }

    /// Offers the game saved on quit, if saves are on and there is one.
    fn offer_saved(&mut self) {
        if self.aids.is_on(Aid::Saves) && saves::read(Slot::OnQuit).is_some() {
            self.aids.set_offer(true);
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
                    self.offer_saved();
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
/// until the window is closed, with `aids` on (#92).
///
/// # Errors
///
/// If the tape can't be read, or the window or its drawing can't be made.
pub fn run(tape: Option<Vec<u8>>, aids: Aids) -> Result<(), String> {
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
        aids,
        journal: Journal::default(),
        set_aside: robin::assists::SetAside::default(),
        overlay: None,
        panel: Panel::new(),
        drawn: None,
        next: Instant::now(),
        period: Duration::from_secs_f64(1.0 / FRAMES_PER_SECOND),
        error: None,
    };
    if app.game.is_some() {
        app.offer_saved();
    }
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.error.map_or(Ok(()), Err)
}
