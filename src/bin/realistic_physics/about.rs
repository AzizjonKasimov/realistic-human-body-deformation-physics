//! The about card: what the simulation is, who made it and how, and where to
//! read more. In the browser the page shows it over the paused simulation
//! (the start screen again, from `web/index.html`, through the page's `rp`
//! gl.js plugin). The desktop app has no page to show, so it opens the write-up
//! of how the project was made in the default browser instead.

/// Shows the about card. True when the card now covers the app, which then
/// waits for it to close.
pub fn show() -> bool {
    platform::show()
}

/// The about card shown by `show` is still open.
pub fn is_open() -> bool {
    platform::is_open()
}

#[cfg(target_arch = "wasm32")]
mod platform {
    extern "C" {
        fn rp_show_about();
        fn rp_about_open() -> i32;
    }

    pub fn show() -> bool {
        // SAFETY: the page provides these through its `rp` gl.js plugin.
        unsafe { rp_show_about() };
        true
    }

    pub fn is_open() -> bool {
        // SAFETY: as above.
        unsafe { rp_about_open() != 0 }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use std::process::Command;

    /// The write-up of how the simulation was made, on the author's site. The
    /// browser page links the same address.
    const PROJECT_PAGE: &str = "https://azizjonkasimov.github.io/work/body-physics/";

    pub fn show() -> bool {
        // Each system's own way to open a link in the default browser. The
        // page is a convenience, so a system without one leaves the app as is.
        #[cfg(target_os = "windows")]
        let opener = "explorer";
        #[cfg(target_os = "macos")]
        let opener = "open";
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let opener = "xdg-open";
        let _ = Command::new(opener).arg(PROJECT_PAGE).spawn();
        false
    }

    pub fn is_open() -> bool {
        false
    }
}
