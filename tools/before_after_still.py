"""Lays two frames of the same moment side by side, before a change and
after it, the thing to look at circled in the before frame with an enlarged
close-up of it, for pictures that show the user a fix.

    python tools/before_after_still.py BEFORE.png AFTER.png OUT.png
        --mark X,Y [--zoom N] [--zoom-corner tl|tr|bl|br]
        [--crop X0,Y0,X1,Y1] [--crop-after X0,Y0,X1,Y1]
        [--scale S] [--before-title TEXT] [--before-note TEXT]
        [--after-title TEXT] [--after-note TEXT]

--mark is where to circle, in the before frame's own pixels (the game's
capture mode prints where each bone showing through unbroken skin lies on
the screen); --zoom the side of the square round it the close-up enlarges
(default 80), and --zoom-corner the corner of the before panel it sits in
(default the one farthest from the mark); --crop the part of each frame to
show (default the whole
frame), --crop-after another for the after frame when the bodies stand
elsewhere in it. Run it from Git Bash, whose Python has Pillow.
"""

import argparse

from PIL import Image, ImageDraw, ImageFont

RED = (255, 60, 60)


def box(text):
    values = [int(value) for value in text.split(",")]
    if len(values) != 4:
        raise argparse.ArgumentTypeError(f"wants X0,Y0,X1,Y1, not {text}")
    return tuple(values)


def point(text):
    values = [float(value) for value in text.split(",")]
    if len(values) != 2:
        raise argparse.ArgumentTypeError(f"wants X,Y, not {text}")
    return tuple(values)


def font(size):
    for name in ("segoeui.ttf", "arial.ttf", "DejaVuSans.ttf"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            pass
    return ImageFont.load_default()


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("before")
    parser.add_argument("after")
    parser.add_argument("out")
    parser.add_argument("--mark", type=point, required=True)
    parser.add_argument("--zoom", type=int, default=80)
    parser.add_argument("--zoom-corner", choices=["tl", "tr", "bl", "br"])
    parser.add_argument("--crop", type=box)
    parser.add_argument("--crop-after", type=box)
    parser.add_argument("--scale", type=float, default=1.1)
    parser.add_argument("--before-title", default="Before the fix (live now)")
    parser.add_argument("--before-note", default="")
    parser.add_argument("--after-title", default="After the fix, same moment")
    parser.add_argument("--after-note", default="")
    args = parser.parse_args()

    before = Image.open(args.before).convert("RGB")
    after = Image.open(args.after).convert("RGB")
    crop = args.crop or (0, 0, before.width, before.height)
    crop_after = args.crop_after or crop
    width = int((crop[2] - crop[0]) * args.scale)
    height = int((crop[3] - crop[1]) * args.scale)
    panel_before = before.crop(crop).resize((width, height), Image.LANCZOS)
    panel_after = after.crop(crop_after).resize((width, height), Image.LANCZOS)

    # The circle on the before panel, and a close-up of what it circles in
    # the panel's lower right corner, joined to it by a line.
    mark = ((args.mark[0] - crop[0]) * args.scale, (args.mark[1] - crop[1]) * args.scale)
    draw = ImageDraw.Draw(panel_before)
    radius = 26
    draw.ellipse(
        [mark[0] - radius, mark[1] - radius, mark[0] + radius, mark[1] + radius],
        outline=RED,
        width=4,
    )
    half = args.zoom / 2
    zoom_box = (
        int(args.mark[0] - half),
        int(args.mark[1] - half),
        int(args.mark[0] + half),
        int(args.mark[1] + half),
    )
    side = 220
    zoom = before.crop(zoom_box).resize((side, side), Image.LANCZOS)
    draw_zoom = ImageDraw.Draw(zoom)
    centre = side / 2
    draw_zoom.ellipse(
        [centre - 30, centre - 30, centre + 30, centre + 30], outline=RED, width=4
    )
    draw_zoom.rectangle([0, 0, side - 1, side - 1], outline=RED, width=3)
    corners = {
        "tl": (10, 10),
        "tr": (width - side - 10, 10),
        "bl": (10, height - side - 10),
        "br": (width - side - 10, height - side - 10),
    }
    name = args.zoom_corner or max(
        corners,
        key=lambda key: (corners[key][0] + side / 2 - mark[0]) ** 2
        + (corners[key][1] + side / 2 - mark[1]) ** 2,
    )
    corner = corners[name]
    # The line runs from the circle's edge to the close-up's nearest corner.
    near = (
        corner[0] + (side if mark[0] > corner[0] + side else 0),
        corner[1] + (side if mark[1] > corner[1] + side else 0),
    )
    toward = (near[0] - mark[0], near[1] - mark[1])
    length = max((toward[0] ** 2 + toward[1] ** 2) ** 0.5, 1.0)
    start = (mark[0] + toward[0] / length * radius, mark[1] + toward[1] / length * radius)
    draw.line([start, near], fill=RED, width=3)
    panel_before.paste(zoom, corner)

    big, small = font(30), font(21)
    gap = 24
    # Each note wrapped to its panel's width.
    measure = ImageDraw.Draw(Image.new("RGB", (1, 1)))

    def wrapped(text):
        lines, line = [], ""
        for word in text.split():
            trial = f"{line} {word}".strip()
            if line and measure.textlength(trial, font=small) > width:
                lines.append(line)
                line = word
            else:
                line = trial
        return lines + ([line] if line else [])

    notes = [wrapped(args.before_note), wrapped(args.after_note)]
    top = 62 + 28 * max(1, *(len(lines) for lines in notes)) + 18
    sheet = Image.new("RGB", (2 * width + 3 * gap, height + top + gap), (24, 24, 28))
    sheet.paste(panel_before, (gap, top))
    sheet.paste(panel_after, (2 * gap + width, top))
    draw_sheet = ImageDraw.Draw(sheet)
    for left, title, lines, colour in [
        (gap, args.before_title, notes[0], (255, 150, 150)),
        (2 * gap + width, args.after_title, notes[1], (170, 230, 170)),
    ]:
        draw_sheet.text((left, 18), title, fill=(255, 255, 255), font=big)
        for row, line in enumerate(lines):
            draw_sheet.text((left, 62 + 28 * row), line, fill=colour, font=small)
    sheet.save(args.out)
    print(args.out, sheet.size)


if __name__ == "__main__":
    main()
