//! Screenshot mode, so the real rendering can be checked without a person at
//! the keyboard. `realistic_physics --capture OUT.png [options]` opens the
//! window, plays an optional scripted strike from
//! `realistic_physics::scenarios`, saves the screen as a PNG, and exits.
//! Native only.
//!
//! ```text
//! --size WXH                 window size (default 1280x720)
//! --view normal|anatomy|both which view to save; `both` writes OUT-normal.png
//!                            and OUT-anatomy.png (default normal)
//! --scenario NAME            play a tuned strike scenario first
//! --strike SPEC              play a custom swing first (strike_scenarios --strike syntax)
//! --frames N                 stop after N steps (default: the whole strike, or
//!                            120 steps of the body settling)
//! --no-ui                    leave out the HUD, control buttons, and pointer ring
//! --label TEXT               write TEXT in the top right corner
//! ```

use std::env;
use std::path::{Path, PathBuf};
use std::process;

use rp::scenarios::{scenario, Scenario, ScenarioExpectations, Strike};

use super::*;

/// Steps the body settles for when no strike is played, as long as the rest test.
const IDLE_FRAMES: i32 = 120;

pub struct CaptureRequest {
    path: PathBuf,
    views: Vec<ViewMode>,
    play: Option<Scenario>,
    frames: Option<i32>,
    hide_ui: bool,
    label: Option<String>,
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
        hide_ui: false,
        label: None,
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
                request.play = Some(Scenario {
                    name: "custom",
                    region: "custom",
                    intent: "custom",
                    strike,
                    followup: None,
                    expectations: ScenarioExpectations::default(),
                });
            }
            "--frames" => {
                let frames = value("--frames");
                request.frames = Some(
                    frames
                        .parse()
                        .unwrap_or_else(|_| fail(&format!("`{frames}` is not a frame count"))),
                );
            }
            "--no-ui" => request.hide_ui = true,
            "--label" => request.label = Some(value("--label")),
            other => fail(&format!("unknown option `{other}`")),
        }
    }
    Some(request)
}

/// Plays the requested strike at fixed steps, then saves each view.
pub async fn run(request: CaptureRequest) {
    // The window settles at its final size over the first frames.
    for _ in 0..4 {
        clear_background(BLACK);
        next_frame().await;
    }
    let (width, height) = (screen_width() as f64, screen_height() as f64);
    let mut app = AppState::new(width, height);
    app.hide_ui = request.hide_ui;
    let dt = app.world.materials().fixed_dt;
    let frames = request
        .frames
        .unwrap_or_else(|| request.play.map_or(IDLE_FRAMES, |play| play.frames()));
    for frame in 0..frames {
        let input = match &request.play {
            Some(play) => play.input(frame, dt, width, height),
            None => rp::InputState::default(),
        };
        if input.active {
            app.tool = input.tool;
            app.impact_power = input.power;
            app.pointer = rp::Vec2 {
                x: input.x,
                y: input.y,
            };
        }
        app.pointer_down = input.down;
        step_world(&mut app, &input, width, height);
    }

    // Draw every view once before saving any: a frame that draws new text
    // grows the glyph atlas partway through and garbles that frame's text, and
    // glyphs uploaded after a screen grab land in the grab's texture instead.
    for &view in &request.views {
        app.view_mode = view;
        draw_view(&app, &request);
        next_frame().await;
    }
    for &view in &request.views {
        app.view_mode = view;
        draw_view(&app, &request);
        let path = view_path(&request.path, view, request.views.len() > 1);
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
    }
}

fn draw_view(app: &AppState, request: &CaptureRequest) {
    draw_app(app);
    if let Some(label) = &request.label {
        draw_label(label);
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

/// OUT.png for a single view, OUT-normal.png and OUT-anatomy.png for both.
fn view_path(path: &Path, view: ViewMode, several: bool) -> PathBuf {
    if !several {
        return path.to_path_buf();
    }
    let stem = path.file_stem().map_or_else(
        || "capture".to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let suffix = match view {
        ViewMode::Normal => "normal",
        ViewMode::Anatomy => "anatomy",
    };
    path.with_file_name(format!("{stem}-{suffix}.png"))
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

fn fail(message: &str) -> ! {
    eprintln!("capture: {message}");
    process::exit(2);
}
