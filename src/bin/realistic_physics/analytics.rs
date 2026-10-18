//! Tells the browser page which tools a visitor strikes the body with, for
//! the live site's visit counts (`web/analytics.js`): each tool once per page
//! load, on its first blow. The desktop app counts nothing.

use realistic_physics as rp;
use std::sync::atomic::{AtomicU8, Ordering};

/// The tools already reported, a bit each.
static REPORTED: AtomicU8 = AtomicU8::new(0);

/// After each simulation step: reports the tool in hand the first time it
/// strikes or presses on the body with the button down. A tool merely
/// carried against the body does not count.
pub fn step(world: &rp::World) {
    if !world.debug().down || !rp::scenarios::tool_touching(world) {
        return;
    }
    let (bit, name) = match world.debug().tool {
        rp::ToolMode::Heavy => (1, "sledgehammer"),
        rp::ToolMode::Blunt => (2, "bat"),
        rp::ToolMode::Sharp => (4, "knife"),
        rp::ToolMode::Fist => (8, "fist"),
    };
    if REPORTED.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
        platform::tool_used(name);
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    extern "C" {
        fn rp_tool_used(name: *const u8, len: usize);
    }

    pub fn tool_used(name: &str) {
        // SAFETY: the page provides this through its `rp` gl.js plugin, which
        // copies the string before it returns.
        unsafe { rp_tool_used(name.as_ptr(), name.len()) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    pub fn tool_used(_name: &str) {}
}
