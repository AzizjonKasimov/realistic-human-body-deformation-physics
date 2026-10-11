//! Finds see-through holes in app captures: places inside the body where the
//! dark background shows through a wound, because nothing drawn covers it.
//!
//! ```text
//! see_through [--min PIXELS] IN.png...
//! ```
//!
//! A pixel is background when it is as dark and as grey as the background,
//! the floor, or anything between (no channel above 40, and channels within
//! 14 of each other). Background joined to the image's border is the world
//! around the body; any other patch of it, of at least `--min` pixels
//! (default 12), is a hole the body encloses. Prints each image's holes,
//! largest first with where they are, and exits 1 if any image has one.
//! Check normal-view captures made with `-NoUi -NoLabel -NoTool`, as
//! `tools\see_through.ps1` does: the anatomy view is see-through on purpose,
//! and buttons, labels, and the tool's dark outline enclose dark patches of
//! their own. A gap between body parts that a limb or a piece of bone closes
//! off counts too, so look at a capture before calling its hole a wound's.
//! Native only.

use macroquad::prelude::{Image, ImageFormat};
use std::env;
use std::fs;
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    let mut min = 12usize;
    let mut inputs = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--min" => {
                min = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .filter(|&value| value > 0)
                    .unwrap_or_else(|| fail("--min needs a positive number"))
            }
            flag if flag.starts_with("--") => fail(&format!("unknown option `{flag}`")),
            path => inputs.push(path.to_string()),
        }
    }
    if inputs.is_empty() {
        fail("usage: see_through [--min PIXELS] IN.png...");
    }

    let mut any = false;
    for path in &inputs {
        let bytes = fs::read(path).unwrap_or_else(|error| fail(&format!("{path}: {error}")));
        let image = Image::from_file_with_format(&bytes, Some(ImageFormat::Png))
            .unwrap_or_else(|error| fail(&format!("{path}: {error}")));
        let holes = holes(&image, min);
        let total: usize = holes.iter().map(|hole| hole.pixels).sum();
        println!("{path}: {} holes, {total} px", holes.len());
        for hole in holes.iter().take(5) {
            println!(
                "  {} px at x {}..{}, y {}..{}",
                hole.pixels, hole.left, hole.right, hole.top, hole.bottom
            );
        }
        any |= !holes.is_empty();
    }
    process::exit(i32::from(any));
}

struct Hole {
    pixels: usize,
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
}

/// Patches of background the body encloses, of at least `min` pixels,
/// largest first.
fn holes(image: &Image, min: usize) -> Vec<Hole> {
    let (width, height) = (image.width as usize, image.height as usize);
    let data = image.get_image_data();
    let background: Vec<bool> = data
        .iter()
        .map(|&[r, g, b, _]| {
            let (low, high) = (r.min(g).min(b), r.max(g).max(b));
            high <= 40 && high - low <= 14
        })
        .collect();
    let mut seen = vec![false; width * height];
    let mut holes = Vec::new();
    let mut stack = Vec::new();
    for start in 0..width * height {
        if !background[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let mut hole = Hole {
            pixels: 0,
            left: usize::MAX,
            right: 0,
            top: usize::MAX,
            bottom: 0,
        };
        let mut touches_border = false;
        while let Some(index) = stack.pop() {
            let (x, y) = (index % width, index / width);
            hole.pixels += 1;
            hole.left = hole.left.min(x);
            hole.right = hole.right.max(x);
            hole.top = hole.top.min(y);
            hole.bottom = hole.bottom.max(y);
            touches_border |= x == 0 || y == 0 || x + 1 == width || y + 1 == height;
            let mut visit = |next: usize| {
                if background[next] && !seen[next] {
                    seen[next] = true;
                    stack.push(next);
                }
            };
            if x > 0 {
                visit(index - 1);
            }
            if x + 1 < width {
                visit(index + 1);
            }
            if y > 0 {
                visit(index - width);
            }
            if y + 1 < height {
                visit(index + width);
            }
        }
        if !touches_border && hole.pixels >= min {
            holes.push(hole);
        }
    }
    holes.sort_by(|a, b| b.pixels.cmp(&a.pixels));
    holes
}

fn fail(message: &str) -> ! {
    eprintln!("see_through: {message}");
    process::exit(2);
}
