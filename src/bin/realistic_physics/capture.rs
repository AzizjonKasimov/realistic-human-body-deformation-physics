//! Screenshot mode, so the real rendering can be checked without a person at
//! the keyboard. `realistic_physics --capture OUT.png [options]` opens the
//! window, plays an optional scripted strike or gesture from
//! `realistic_physics::scenarios`, saves the screen as a PNG, and exits.
//! Native only.
//!
//! ```text
//! --size WXH                 window size (default 1280x720)
//! --view normal|anatomy|both which view to save; `both` writes OUT-normal.png
//!                            and OUT-anatomy.png (default normal). The anatomy
//!                            view shows muscle and bones through see-through
//!                            skin, for checking; players only get the normal view.
//! --scenario NAME            play a tuned strike scenario first
//! --strike SPEC              play a custom swing first (strike_scenarios --strike syntax)
//! --gesture SPEC             play a gesture first (strike_scenarios --gesture syntax)
//! --frames N                 stop after N steps (default: the whole strike, or
//!                            120 steps of the body settling)
//! --every N                  also save the screen every N steps, as OUT-STEP.png,
//!                            to see how things move
//! --no-ui                    leave out the HUD, control buttons, and pointer ring
//! --label TEXT               write TEXT in the bottom right corner
//! --no-label                 write no label or step number, for frames that
//!                            become a video (`--every 1` saves every step)
//! --body-frame X,Y,H         put the top of the head at X,Y and make the body
//!                            H pixels tall instead of fitting it to the window.
//!                            Injuries depend on the body's size in pixels, and
//!                            the tuned scenarios use 561.6, its height in a
//!                            1280x720 window, so a tall video keeps that size.
//! ```

use std::env;
use std::path::{Path, PathBuf};
use std::process;

use rp::scenarios::{scenario, Gesture, Play, Scenario, Strike};

use super::*;

/// Steps the body settles for when no strike is played, as long as the rest test.
const IDLE_FRAMES: i32 = 120;

pub struct CaptureRequest {
    path: PathBuf,
    views: Vec<ViewMode>,
    play: Option<Scenario>,
    frames: Option<i32>,
    every: Option<i32>,
    hide_ui: bool,
    label: Option<String>,
    show_label: bool,
    body_frame: Option<rp::BodyFrame>,
}

/// The window size asked for with `--size`, when capturing.
pub fn window_size() -> Option<(i32, i32)> {
    let args: Vec<String> = env::args().collect();
    if !args.iter().any(|arg| arg == "--capture") {
        return None;
    }
    let size = args
        .iter()
        .position(|arg| arg == "--size")
        .and_then(|index| args.get(index + 1))?;
    match parse_size(size) {
        Ok(size) => Some(size),
        Err(message) => fail(&message),
    }
}

/// The capture asked for on the command line, if any. Exits on bad options.
pub fn request() -> Option<CaptureRequest> {
    if !env::args().any(|arg| arg == "--capture") {
        return None;
    }
    let mut args = env::args().skip(1);
    let mut request = CaptureRequest {
        path: PathBuf::new(),
        views: vec![ViewMode::Normal],
        play: None,
        frames: None,
        every: None,
        hide_ui: false,
        label: None,
        show_label: true,
        body_frame: None,
    };
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .unwrap_or_else(|| fail(&format!("{name} needs a value")))
        };
        match arg.as_str() {
            "--capture" => request.path = PathBuf::from(value("--capture")),
            "--size" => {
                value("--size");
            }
            "--view" => {
                request.views = match value("--view").as_str() {
                    "normal" => vec![ViewMode::Normal],
                    "anatomy" => vec![ViewMode::Anatomy],
                    "both" => vec![ViewMode::Normal, ViewMode::Anatomy],
                    other => fail(&format!("unknown view `{other}`")),
                }
            }
            "--scenario" => {
                let name = value("--scenario");
                request.play = Some(
                    scenario(&name).unwrap_or_else(|| fail(&format!("unknown scenario `{name}`"))),
                );
            }
            "--strike" => {
                let strike = Strike::parse(&value("--strike")).unwrap_or_else(|error| fail(&error));
                request.play = Some(Scenario::custom(Play::Swing(strike, None)));
            }
            "--gesture" => {
                let gesture =
                    Gesture::parse(&value("--gesture")).unwrap_or_else(|error| fail(&error));
                request.play = Some(Scenario::custom(Play::Gesture(gesture)));
            }
            "--frames" => request.frames = Some(steps(&value("--frames"))),
            "--every" => request.every = Some(steps(&value("--every")).max(1)),
            "--no-ui" => request.hide_ui = true,
            "--label" => request.label = Some(value("--label")),
            "--no-label" => request.show_label = false,
            "--body-frame" => {
                request.body_frame =
                    Some(parse_body_frame(&value("--body-frame")).unwrap_or_else(|e| fail(&e)));
            }
            other => fail(&format!("unknown option `{other}`")),
        }
    }
    Some(request)
}

/// Plays the requested strike or gesture at fixed steps, then saves each view;
/// with `--every`, also on the way.
pub async fn run(request: CaptureRequest) {
    // The window settles at its final size over the first frames.
    for _ in 0..4 {
        clear_background(BLACK);
        next_frame().await;
    }
    warm_glyphs().await;
    let (width, height) = (screen_width() as f64, screen_height() as f64);
    let mut app = AppState::new(width, height);
    if let Some(frame) = request.body_frame {
        app.place_body(frame, width as f32, height as f32);
    }
    app.hide_ui = request.hide_ui;
    let dt = app.world.materials().fixed_dt;
    let frames = request
        .frames
        .unwrap_or_else(|| request.play.map_or(IDLE_FRAMES, |play| play.frames()));
    for frame in 0..frames {
        let input = match &request.play {
            Some(play) => play.input(frame, dt, app.frame),
            None => rp::InputState::default(),
        };
        if input.active {
            app.tool = input.tool;
            app.pointer = rp::Vec2 {
                x: input.x,
                y: input.y,
            };
        }
        app.pointer_down = input.down;
        step_world(&mut app, &input, width, height);
        let step = frame + 1;
        if request
            .every
            .is_some_and(|every| step % every == 0 && step < frames)
        {
            save_views(&mut app, &request, Some(step)).await;
        }
    }
    save_views(&mut app, &request, request.every.map(|_| frames)).await;
}

/// Draws every character the app and the label use at their text sizes once,
/// before any screen grab: a frame that draws new text grows the glyph atlas
/// partway through and garbles that frame's text, and glyphs uploaded after a
/// screen grab land in the grab's texture instead.
async fn warm_glyphs() {
    let characters: String = (' '..='~').collect();
    for size in [15.0, 17.0, 20.0] {
        draw_text(&characters, 0.0, size, size, WHITE);
    }
    next_frame().await;
    clear_background(BLACK);
    next_frame().await;
}

/// Saves each requested view of the app as it is now; `step` names the files
/// of a capture taken on the way.
async fn save_views(app: &mut AppState, request: &CaptureRequest, step: Option<i32>) {
    // Draw every view once before saving any, so text drawn for the first
    // time does not garble the saved frame.
    for &view in &request.views {
        app.view_mode = view;
        draw_view(app, request, step);
        next_frame().await;
    }
    for &view in &request.views {
        app.view_mode = view;
        draw_view(app, request, step);
        let path = view_path(&request.path, view, request.views.len() > 1, step);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                fail(&format!("cannot create {}: {error}", parent.display()))
            });
        }
        get_screen_data().export_png(&path.to_string_lossy());
        println!("wrote {}", path.display());
        next_frame().await;
    }
}

fn draw_view(app: &AppState, request: &CaptureRequest, step: Option<i32>) {
    draw_app(app);
    if !request.show_label {
        return;
    }
    match (&request.label, step) {
        (Some(label), Some(step)) => draw_label(&format!("{label} step {step}")),
        (Some(label), None) => draw_label(label),
        (None, Some(step)) => draw_label(&format!("step {step}")),
        (None, None) => {}
    }
}

/// The label sits in the empty floor strip at the bottom right, clear of the
/// HUD and the control buttons at any window size.
fn draw_label(text: &str) {
    let size = 20.0;
    let measure = measure_text(text, None, size as u16, 1.0);
    let x = screen_width() - measure.width - 14.0;
    let baseline = screen_height() - 12.0;
    draw_rectangle(
        x - 6.0,
        baseline - size + 2.0,
        measure.width + 12.0,
        size + 6.0,
        rgba(13, 13, 15, 230),
    );
    draw_text(text, x, baseline, size, rgba(232, 226, 212, 255));
}

/// OUT.png for a single view, OUT-normal.png and OUT-anatomy.png for both,
/// with the step number after the name for captures taken on the way
/// (OUT-0012.png).
fn view_path(path: &Path, view: ViewMode, several: bool, step: Option<i32>) -> PathBuf {
    if !several && step.is_none() {
        return path.to_path_buf();
    }
    let mut name = path.file_stem().map_or_else(
        || "capture".to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    if let Some(step) = step {
        name.push_str(&format!("-{step:04}"));
    }
    if several {
        name.push_str(match view {
            ViewMode::Normal => "-normal",
            ViewMode::Anatomy => "-anatomy",
        });
    }
    path.with_file_name(format!("{name}.png"))
}

fn steps(text: &str) -> i32 {
    text.parse()
        .ok()
        .filter(|&steps: &i32| steps >= 0)
        .unwrap_or_else(|| fail(&format!("`{text}` is not a step count")))
}

fn parse_size(text: &str) -> Result<(i32, i32), String> {
    let (width, height) = text
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("`{text}` should look like 1280x720"))?;
    let number = |n: &str| {
        n.trim()
            .parse::<i32>()
            .ok()
            .filter(|&n| n >= 64)
            .ok_or_else(|| format!("`{n}` is not a window size"))
    };
    Ok((number(width)?, number(height)?))
}

fn parse_body_frame(text: &str) -> Result<rp::BodyFrame, String> {
    let numbers: Vec<f64> = text
        .split(',')
        .map(|n| n.trim().parse::<f64>().ok().filter(|n| n.is_finite()))
        .collect::<Option<_>>()
        .ok_or_else(|| format!("`{text}` should look like 288,240,561.6"))?;
    match numbers[..] {
        [x, y, height] if height >= 64.0 => Ok(rp::BodyFrame {
            origin: rp::Vec2 { x, y },
            height,
        }),
        _ => Err(format!(
            "`{text}` should be X,Y,HEIGHT with HEIGHT at least 64"
        )),
    }
}

fn fail(message: &str) -> ! {
    eprintln!("capture: {message}");
    process::exit(2);
}
