//! Lays PNG screenshots out in a grid in one image, so several captures can be
//! looked at in one go:
//!
//! ```text
//! contact_sheet OUT.png [--height H] [--columns N] [--crop X,Y,W,H] IN.png...
//! ```
//!
//! Every image is cut to `--crop`'s rectangle, in its own pixels from the top
//! left, if given, so a sheet can show just the body, then scaled to the same
//! height (default 540 px) and placed left to right, `--columns` per row
//! (default: all in one row). Native only.

use macroquad::prelude::{Image, ImageFormat};
use std::env;
use std::fs;
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    let mut output = None;
    let mut height = 540usize;
    let mut columns = usize::MAX;
    let mut crop = None;
    let mut inputs = Vec::new();
    while let Some(arg) = args.next() {
        let mut number = |name: &str| -> usize {
            args.next()
                .and_then(|value| value.parse().ok())
                .filter(|&value| value > 0)
                .unwrap_or_else(|| fail(&format!("{name} needs a positive number")))
        };
        match arg.as_str() {
            "--height" => height = number("--height"),
            "--columns" => columns = number("--columns"),
            "--crop" => crop = Some(parse_crop(args.next())),
            flag if flag.starts_with("--") => fail(&format!("unknown option `{flag}`")),
            path if output.is_none() => output = Some(path.to_string()),
            path => inputs.push(path.to_string()),
        }
    }
    let output = output.unwrap_or_else(|| {
        fail("usage: contact_sheet OUT.png [--height H] [--columns N] [--crop X,Y,W,H] IN.png...")
    });
    if inputs.is_empty() {
        fail("no input images");
    }

    let gap = 8;
    let tiles: Vec<(usize, usize, Vec<[u8; 4]>)> = inputs
        .iter()
        .map(|path| {
            let bytes = fs::read(path).unwrap_or_else(|error| fail(&format!("{path}: {error}")));
            let image = Image::from_file_with_format(&bytes, Some(ImageFormat::Png))
                .unwrap_or_else(|error| fail(&format!("{path}: {error}")));
            scaled(&image, crop, height)
        })
        .collect();
    let rows: Vec<&[(usize, usize, Vec<[u8; 4]>)]> =
        tiles.chunks(columns.min(tiles.len())).collect();
    let sheet_width = rows
        .iter()
        .map(|row| row.iter().map(|tile| tile.0).sum::<usize>() + gap * (row.len() - 1))
        .max()
        .unwrap_or(1);
    let sheet_height = height * rows.len() + gap * (rows.len() - 1);
    if sheet_width > u16::MAX as usize || sheet_height > u16::MAX as usize {
        fail("contact sheet would be too large; lower --height or set --columns");
    }

    // Image rows run bottom up, the way export_png expects screen grabs.
    let mut pixels = vec![[46u8, 46, 50, 255]; sheet_width * sheet_height];
    let mut top = 0;
    for row in &rows {
        let mut left = 0;
        for (width, tile_height, tile) in row.iter() {
            for y in 0..*tile_height {
                let sheet_row = sheet_height - 1 - (top + y);
                let start = sheet_row * sheet_width + left;
                pixels[start..start + width].copy_from_slice(&tile[y * width..(y + 1) * width]);
            }
            left += width + gap;
        }
        top += height + gap;
    }
    let image = Image {
        bytes: pixels.into_iter().flatten().collect(),
        width: sheet_width as u16,
        height: sheet_height as u16,
    };
    image.export_png(&output);
    println!("wrote {output} ({} images)", inputs.len());
}

/// A rectangle of an image, in its pixels from the top left.
#[derive(Clone, Copy)]
struct Crop {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

/// Reads `--crop X,Y,W,H`.
fn parse_crop(spec: Option<String>) -> Crop {
    let values: Option<Vec<usize>> = spec.and_then(|spec| {
        spec.split(',')
            .map(|value| value.trim().parse().ok())
            .collect()
    });
    match values.as_deref() {
        Some(&[x, y, width, height]) if width > 0 && height > 0 => Crop {
            x,
            y,
            width,
            height,
        },
        _ => fail("--crop needs X,Y,W,H in pixels, W and H above 0"),
    }
}

/// The image, or its `crop` rectangle where it lies on the image, scaled to
/// `height` rows (box-filtered), top row first.
fn scaled(image: &Image, crop: Option<Crop>, height: usize) -> (usize, usize, Vec<[u8; 4]>) {
    let (image_width, image_height) = (image.width as usize, image.height as usize);
    let crop = crop.unwrap_or(Crop {
        x: 0,
        y: 0,
        width: image_width,
        height: image_height,
    });
    let (left, top) = (crop.x.min(image_width - 1), crop.y.min(image_height - 1));
    let source_width = crop.width.min(image_width - left);
    let source_height = crop.height.min(image_height - top);
    let width = (source_width * height / source_height.max(1)).max(1);
    let source = image.get_image_data();
    let mut out = Vec::with_capacity(width * height);
    for y in 0..height {
        let y0 = y * source_height / height;
        let y1 = ((y + 1) * source_height / height).max(y0 + 1);
        for x in 0..width {
            let x0 = x * source_width / width;
            let x1 = ((x + 1) * source_width / width).max(x0 + 1);
            let mut sum = [0u32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let pixel = source[(top + sy) * image_width + left + sx];
                    for channel in 0..4 {
                        sum[channel] += pixel[channel] as u32;
                    }
                }
            }
            let count = ((y1 - y0) * (x1 - x0)) as u32;
            out.push(sum.map(|channel| (channel / count) as u8));
        }
    }
    (width, height, out)
}

fn fail(message: &str) -> ! {
    eprintln!("contact_sheet: {message}");
    process::exit(2);
}
