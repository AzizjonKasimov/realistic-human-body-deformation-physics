//! Self-test mode: plays every tuned scenario from
//! `realistic_physics::scenarios` at fixed simulation steps, checks each against
//! its bands, times the steps, and reports the results. It does not depend on
//! the frame rate, so it also works in a browser tab the browser is throttling.
//!
//! Open the browser build with `?selftest`: the page starts it at once and
//! puts the report in `window.__selftest` and its status line. Natively,
//! `realistic_physics --selftest` prints the report as JSON and exits.

use rp::scenarios::{run, scenarios, ScenarioResult, SCENARIO_HEIGHT, SCENARIO_WIDTH};

use super::*;

/// The self-test was asked for.
pub fn requested() -> bool {
    platform::selftest_requested()
}

struct Outcome {
    name: &'static str,
    result: ScenarioResult,
    violations: Vec<String>,
}

/// Plays every scenario, reports, then shows the results (natively it exits
/// after reporting).
pub async fn run_all() {
    let all = scenarios();
    let mut outcomes = Vec::with_capacity(all.len());
    let (mut steps, mut total_ms, mut slowest_ms) = (0usize, 0.0f64, 0.0f64);
    for (index, scenario) in all.iter().enumerate() {
        clear_background(rgba(16, 16, 18, 255));
        draw_line_of_text(
            &format!("Self-test: {} ({}/{})", scenario.name, index + 1, all.len()),
            0,
            WHITE,
        );
        next_frame().await;

        let mut last = platform::now_ms();
        let (_, result) = run(scenario, SCENARIO_WIDTH, SCENARIO_HEIGHT, |frame, _, _| {
            let now = platform::now_ms();
            // The first interval also covers building the body.
            if frame > 0 {
                let step = now - last;
                steps += 1;
                total_ms += step;
                slowest_ms = slowest_ms.max(step);
            }
            last = now;
        });
        let violations = scenario.violations(&result);
        outcomes.push(Outcome {
            name: scenario.name,
            result,
            violations,
        });
    }

    let average_ms = total_ms / steps.max(1) as f64;
    platform::report(&report_json(&outcomes, steps, average_ms, slowest_ms));
    if !cfg!(target_arch = "wasm32") {
        return;
    }
    loop {
        draw_results(&outcomes, average_ms, slowest_ms);
        next_frame().await;
    }
}

fn report_json(outcomes: &[Outcome], steps: usize, average_ms: f64, slowest_ms: f64) -> String {
    let passed = outcomes.iter().all(|outcome| outcome.violations.is_empty());
    let scenarios: Vec<String> = outcomes
        .iter()
        .map(|outcome| {
            let r = &outcome.result;
            let violations: Vec<String> = outcome
                .violations
                .iter()
                .map(|violation| format!("\"{}\"", escape(violation)))
                .collect();
            format!(
                "{{\"name\":\"{}\",\"in_band\":{},\"violations\":[{}],\"bones\":{},\"ribs\":{},\"skin\":{},\"muscle\":{},\"bruises\":{},\"fluid\":{},\"blood_loss\":{:.5},\"max_tool_turn\":{:.2},\"fastest_tissue\":{:.0}}}",
                escape(outcome.name),
                outcome.violations.is_empty(),
                violations.join(","),
                r.bone_fractures,
                r.rib_fractures,
                r.skin_tears,
                r.muscle_tears,
                r.contusion_events,
                r.fluid_emitted,
                r.blood_loss,
                r.max_tool_turn,
                r.max_point_speed
            )
        })
        .collect();
    format!(
        "{{\"passed\":{passed},\"platform\":\"{}\",\"steps\":{steps},\"avg_step_ms\":{average_ms:.3},\"max_step_ms\":{slowest_ms:.3},\"scenarios\":[{}]}}",
        if cfg!(target_arch = "wasm32") {
            "wasm"
        } else {
            "native"
        },
        scenarios.join(",")
    )
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn draw_results(outcomes: &[Outcome], average_ms: f64, slowest_ms: f64) {
    clear_background(rgba(16, 16, 18, 255));
    let failed = outcomes
        .iter()
        .filter(|outcome| !outcome.violations.is_empty())
        .count();
    let (headline, color) = if failed == 0 {
        (
            format!("Self-test passed: {} scenarios in band", outcomes.len()),
            rgba(94, 176, 108, 255),
        )
    } else {
        (
            format!(
                "Self-test FAILED: {failed} of {} scenarios out of band",
                outcomes.len()
            ),
            rgba(211, 93, 70, 255),
        )
    };
    draw_line_of_text(&headline, 0, color);
    draw_line_of_text(
        &format!("simulation step {average_ms:.2} ms on average, {slowest_ms:.2} ms at most"),
        1,
        WHITE,
    );
    for (index, outcome) in outcomes.iter().enumerate() {
        let r = &outcome.result;
        let line = format!(
            "{} {}: bones {}, skin {}, bruises {}, fluid {}{}",
            if outcome.violations.is_empty() {
                "ok  "
            } else {
                "FAIL"
            },
            outcome.name,
            r.bone_fractures,
            r.skin_tears,
            r.contusion_events,
            r.fluid_emitted,
            outcome
                .violations
                .first()
                .map_or(String::new(), |violation| format!(" ({violation})"))
        );
        let color = if outcome.violations.is_empty() {
            rgba(200, 196, 186, 255)
        } else {
            rgba(211, 93, 70, 255)
        };
        draw_line_of_text(&line, index + 3, color);
    }
}

fn draw_line_of_text(text: &str, line: usize, color: Color) {
    draw_text(text, 16.0, 28.0 + line as f32 * 22.0, 20.0, color);
}

#[cfg(target_arch = "wasm32")]
mod platform {
    extern "C" {
        fn rp_selftest_requested() -> i32;
        fn rp_now_ms() -> f64;
        fn rp_selftest_report(json: *const u8, len: usize);
    }

    pub fn selftest_requested() -> bool {
        // SAFETY: the page provides these through the `rp` gl.js plugin.
        unsafe { rp_selftest_requested() != 0 }
    }

    pub fn now_ms() -> f64 {
        // SAFETY: as above.
        unsafe { rp_now_ms() }
    }

    pub fn report(json: &str) {
        // SAFETY: as above; the page copies the string before this returns.
        unsafe { rp_selftest_report(json.as_ptr(), json.len()) }
    }

    /// gl.js checks the page's `rp` plugin version against this.
    #[no_mangle]
    pub extern "C" fn rp_crate_version() -> u32 {
        1
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use std::sync::OnceLock;
    use std::time::Instant;

    pub fn selftest_requested() -> bool {
        std::env::args().any(|arg| arg == "--selftest")
    }

    pub fn now_ms() -> f64 {
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
    }

    pub fn report(json: &str) {
        println!("{json}");
    }
}
