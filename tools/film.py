"""Turns a run of capture frames into a short film (an animated GIF) to show a
person how something moves, which a single frame or a sheet hides.

    python tools/film.py OUT.gif FRAMES... [--crop X,Y,W,H] [--ms MS]
        [--width PX] [--label TEXT] [--mark X,Y]... [--below FRAMES...]
        [--below-label TEXT]

FRAMES are PNGs in play order, such as `capture.ps1 -Every 2` writes them
(output/captures/NAME-1280x720-0002.png, ...); a glob is expanded in
order, so `output/captures/crossing-1280x720-*.png` works from PowerShell
too. `--crop` cuts each frame to that rectangle in its own pixels, `--ms`
is how long each frame shows (67 plays frames saved every 2 steps at half
speed), `--width` scales the film to that many pixels wide, `--label`
writes a caption in the corner, and `--mark X,Y` draws a small white tick
at that pixel of the frame (before cropping), such as where a foot stood.
With `--below`, a second run of frames plays under the first, frame by
frame, as a before-and-after film (`--label` names the top row and
`--below-label` the bottom one); the shorter run holds its last frame.

Needs Pillow (`python -m pip install pillow`).
"""

import argparse
import glob
import sys

from PIL import Image, ImageDraw


def expand(patterns):
    """The frames named, each glob expanded in order."""
    frames = []
    for pattern in patterns:
        found = sorted(glob.glob(pattern))
        frames.extend(found if found else [pattern])
    return frames


def numbers(text, count, name):
    parts = [float(part) for part in text.split(",")]
    if len(parts) != count:
        sys.exit(f"{name} takes {count} numbers, got `{text}`")
    return parts


def frame_image(path, crop, marks, width, label):
    image = Image.open(path).convert("RGB")
    draw = ImageDraw.Draw(image)
    for x, y in marks:
        draw.line([(x, y - 9), (x, y + 9)], fill=(255, 255, 255), width=2)
    if crop:
        x, y, w, h = crop
        image = image.crop((x, y, x + w, y + h))
    if width and image.width != width:
        height = round(image.height * width / image.width)
        image = image.resize((width, height), Image.LANCZOS)
    if label:
        draw = ImageDraw.Draw(image)
        draw.rectangle([(0, 0), (8 + 7 * len(label), 18)], fill=(0, 0, 0))
        draw.text((4, 3), label, fill=(255, 255, 255))
    return image


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("out")
    parser.add_argument("frames", nargs="+")
    parser.add_argument("--crop")
    parser.add_argument("--ms", type=int, default=67)
    parser.add_argument("--width", type=int, default=0)
    parser.add_argument("--label", default="")
    parser.add_argument("--mark", action="append", default=[])
    parser.add_argument("--below", nargs="+", default=[])
    parser.add_argument("--below-label", default="")
    args = parser.parse_args()

    crop = [round(v) for v in numbers(args.crop, 4, "--crop")] if args.crop else None
    marks = [numbers(mark, 2, "--mark") for mark in args.mark]
    top = expand(args.frames)
    below = expand(args.below)
    if not top:
        sys.exit("no frames to film")
    films = []
    for index in range(max(len(top), len(below))):
        image = frame_image(top[min(index, len(top) - 1)], crop, marks, args.width, args.label)
        if below:
            under = frame_image(
                below[min(index, len(below) - 1)], crop, marks, args.width, args.below_label
            )
            both = Image.new("RGB", (image.width, image.height + under.height + 4), (60, 60, 60))
            both.paste(image, (0, 0))
            both.paste(under, (0, image.height + 4))
            image = both
        films.append(image)
    films[0].save(
        args.out,
        save_all=True,
        append_images=films[1:],
        duration=args.ms,
        loop=0,
        optimize=True,
    )
    print(f"wrote {args.out} ({len(films)} frames, {len(films) * args.ms / 1000:.1f} s)")


if __name__ == "__main__":
    main()
