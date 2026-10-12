#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy>=2", "pillow>=11"]
# ///
"""Build the FISHING MINIGAME trailer into out/fishing_minigame_trailer.mp4.

    uv run build.py              everything that's missing or out of date
    uv run build.py --refilm     film the game's takes again (after the game changes)
    uv run build.py --sheets     contact sheets of every take, to check the cuts in edit.py
    uv run build.py --still 9.5  one frame of the trailer, as out/still-09.50.png
    uv run build.py --preview    half size and 15 fps, quick to check the edit

The edit (what plays when) is in edit.py. The downloads (music, font), the filmed takes and the
Blender ending are kept in cache/; delete a folder there to make it again.
"""

import argparse
import hashlib
import json
import math
import os
import runpy
import shutil
import subprocess
import sys
import urllib.parse
import urllib.request
from collections import OrderedDict
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

import edit

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CACHE = HERE / "cache"
OUT = HERE / "out"
W, H = edit.SIZE
FPS = edit.FPS
SR = 48000  # output sample rate, Hz
TAKE_HZ = 60  # the takes' frames a second of game time
SCREEN = 466  # panel size, px
FONT_URL = "https://github.com/google/fonts/raw/main/apache/luckiestguy/LuckiestGuy-Regular.ttf"
CASE_STL = ROOT / "case" / "out" / "reel_case.stl"
CASE_PARAMS = ROOT / "case" / "params.py"
BLENDERS = [
    os.environ.get("BLENDER", ""),
    shutil.which("blender") or "",
    "/Applications/Blender.app/Contents/MacOS/Blender",
    str(
        Path.home() / "Library/Application Support/Steam/steamapps/common/Blender/Blender.app"
        "/Contents/MacOS/Blender"
    ),
]
INK = "#1d1a3a"  # letter outlines
CHEW_HZ = 2.4  # how fast the man chews, in bites a second
SKY_TOP, SKY_BOTTOM = "#6fbdf7", "#d9f1ff"


def run(cmd, **kwargs):
    print("  $", " ".join(str(c) for c in cmd))
    subprocess.run([str(c) for c in cmd], check=True, **kwargs)


def rgb(hex_color):
    hex_color = hex_color.lstrip("#")
    return tuple(int(hex_color[i : i + 2], 16) for i in (0, 2, 4))


def clamp(x, lo=0.0, hi=1.0):
    return max(lo, min(hi, x))


def smooth(x):
    """Smoothstep: 0 to 1, easing in and out."""
    x = clamp(x)
    return x * x * (3 - 2 * x)


def back_out(x):
    """0 to 1 with a little overshoot: the pop letters and labels make."""
    x = clamp(x)
    c1 = 2.2
    return 1 + (c1 + 1) * (x - 1) ** 3 + c1 * (x - 1) ** 2


# --- fetching ---------------------------------------------------------------------------------


def download(urls, dest):
    """Fetch `dest` from the first of `urls` that works, unless it's already there."""
    if dest.exists():
        return dest
    dest.parent.mkdir(parents=True, exist_ok=True)
    print(f"fetching {dest.name}")
    tmp = dest.with_suffix(dest.suffix + ".part")
    for url in urls:
        try:
            request = urllib.request.Request(url, headers={"User-Agent": "logic-orbit-trailer"})
            with urllib.request.urlopen(request, timeout=300) as response, open(tmp, "wb") as f:
                shutil.copyfileobj(response, f)
            tmp.rename(dest)
            return dest
        except OSError as err:
            print(f"  {url}: {err}")
    sys.exit(f"could not download {dest.name}; put a copy at {dest}")


def archive_urls(path):
    """Download URLs for `path` in FreePD's library, which lives on in archive.org's `freepd`."""
    quoted = urllib.parse.quote(path)
    urls = []
    try:
        with urllib.request.urlopen("https://archive.org/metadata/freepd", timeout=60) as response:
            meta = json.load(response)
        urls = [f"https://{server}{meta['dir']}/{quoted}" for server in meta["workable_servers"]]
    except (OSError, KeyError, ValueError) as err:
        print(f"  archive.org metadata: {err}")
    return urls + [f"https://archive.org/download/freepd/{quoted}"]


def fetch_assets():
    """The font and each music track, downloaded into cache/assets/ once."""
    font = download([FONT_URL], CACHE / "assets" / "LuckiestGuy-Regular.ttf")
    music = {}
    for name, track in edit.MUSIC.items():
        if "archive" in track:
            urls, suffix = archive_urls(track["archive"]), Path(track["archive"]).suffix
        else:
            urls, suffix = [track["url"]], Path(track["url"]).suffix
        music[name] = download(urls, CACHE / "assets" / f"music-{name}{suffix}")
    return font, music


# --- audio ------------------------------------------------------------------------------------


def load_audio(path, channels=2):
    """Any audio file as float32 samples at SR, decoded by ffmpeg."""
    raw = subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(path),
            "-ac",
            str(channels),
            "-ar",
            str(SR),
            "-f",
            "f32le",
            "-",
        ],
        check=True,
        capture_output=True,
    ).stdout
    return np.frombuffer(raw, np.float32).reshape(-1, channels).copy()


def db(gain_db):
    """dB to a linear gain."""
    return 10 ** (gain_db / 20)


def first_sound(track, threshold=0.02):
    """Seconds to just before `track` first gets louder than `threshold`."""
    loud = np.flatnonzero(np.abs(track).max(axis=1) > threshold)
    return max(0.0, loud[0] / SR - 0.02) if len(loud) else 0.0


def noise_burst(rng, secs, cutoff, decay):
    """White noise through a one-pole low-pass, dying away: the grains of a crunch."""
    n = int(secs * SR)
    noise = rng.standard_normal(n).astype(np.float32)
    alpha = 1 - math.exp(-2 * math.pi * cutoff / SR)
    out = np.empty(n, np.float32)
    acc = 0.0
    for i in range(n):
        acc += alpha * (noise[i] - acc)
        out[i] = acc
    out -= np.convolve(out, np.ones(24) / 24, "same")
    return out * np.exp(-np.arange(n) / (decay * SR)).astype(np.float32)


def crunch(seed):
    """A crunch: 14 random noise bursts in 0.42 s, normalized."""
    rng = np.random.default_rng(seed)
    out = np.zeros(int(0.42 * SR), np.float32)
    for _ in range(14):
        burst = noise_burst(rng, 0.06, rng.uniform(2500, 7000), rng.uniform(0.008, 0.02))
        at = int(rng.uniform(0, 0.3) * SR)
        out[at : at + len(burst)] += burst[: len(out) - at] * rng.uniform(0.4, 1.0)
    return out / (np.abs(out).max() + 1e-9)


def thump():
    """The low knock under a bite."""
    tau = np.arange(int(0.18 * SR)) / SR
    return (np.sin(2 * math.pi * (95 - 120 * tau) * tau) * np.exp(-tau / 0.045)).astype(np.float32)


def whoosh(rise, tail=0.6):
    """Air rushing past: brightening for `rise` seconds, then dying away over `tail`."""
    rng = np.random.default_rng(5)
    peak, n = int(rise * SR), int((rise + tail) * SR)
    noise = rng.standard_normal(n).astype(np.float32)
    out = np.empty(n, np.float32)
    acc = 0.0
    for i in range(n):
        if i < peak:
            cutoff = 300 + 5000 * (i / peak) ** 2
        else:
            cutoff = 5300 - 3800 * (i - peak) / (n - peak)
        acc += (1 - math.exp(-2 * math.pi * cutoff / SR)) * (noise[i] - acc)
        out[i] = acc
    tau = np.arange(n) / SR
    out *= np.where(tau < rise, (tau / rise) ** 2, np.exp(-(tau - rise) / (tail / 4)))
    return out / (np.abs(out).max() + 1e-9)


def ring_out(sound, cut, most=1.0):
    """Where a sound playing at sample `cut` falls silent, up to `most` seconds on: a cut
    lets the game's sounds finish instead of chopping them."""
    end = min(len(sound), cut + int(most * SR))
    window = SR // 100
    for i in range(cut, end, window):
        if np.abs(sound[i : i + window]).max() < 1e-4:
            return i
    return end


def level(points, start, n):
    """Gains for `n` samples from `start` seconds, along (time, dB) points."""
    times = start + np.arange(n) / SR
    return db(np.interp(times, [t for t, _ in points], [d for _, d in points])).astype(np.float32)


def scratch(track, at, secs=0.6):
    """A record scratch: the track dragged back and forth under a hand, slowing to a stop."""
    n = int(secs * SR)
    tau = np.arange(n) / SR
    position = at + 0.16 * np.sin(2 * math.pi * 3.2 * tau) * (1 - tau / secs) + 0.08 * tau
    index = position * SR
    out = np.stack([np.interp(index, np.arange(len(track)), track[:, c]) for c in (0, 1)], axis=1)
    return (out * np.clip((secs - tau) / 0.1, 0, 1)[:, None]).astype(np.float32)


# --- the takes --------------------------------------------------------------------------------


class Takes:
    """The filmed takes: frames by time, their sounds, and where the player stands."""

    def __init__(self, root):
        self.root = root
        self.counts = {}
        self.cache = OrderedDict()

    def count(self, take):
        """How many frames a take has; exits if it was never filmed."""
        if take not in self.counts:
            self.counts[take] = len(list((self.root / take).glob("*.png")))
            if not self.counts[take]:
                sys.exit(f"no footage for take {take!r}: run with --refilm")
        return self.counts[take]

    def path(self, take, secs):
        """The frame file `secs` into a take, clamped to its ends."""
        index = min(max(round(secs * TAKE_HZ), 0), self.count(take) - 1)
        return self.root / take / f"{index:05d}.png"

    def frame(self, take, secs):
        """That frame as an image; the last 64 loaded are kept."""
        path = self.path(take, secs)
        if path not in self.cache:
            self.cache[path] = Image.open(path).convert("RGB")
            if len(self.cache) > 64:
                self.cache.popitem(last=False)
        return self.cache[path]

    def audio(self, take):
        return load_audio(self.root / take / "audio.wav")

    def focus(self, take):
        """Where the player stands in the take (their feet), from `focus.txt`."""
        x, y = (float(v) for v in (self.root / take / "focus.txt").read_text().split())
        return x, y


def film(refilm):
    """Film every take if `refilm` or if any take the edit uses is missing."""
    footage = CACHE / "footage"
    wanted = {take for shot in edit.SHOTS for take, *_ in shot.get("takes", [])}
    wanted |= {shot["screen"][0] for shot in edit.SHOTS if "screen" in shot}
    missing = [take for take in sorted(wanted) if not (footage / take / "audio.wav").exists()]
    if refilm or missing:
        print("filming the takes")
        run(
            ["cargo", "run", "--release", "-p", "orbit_desktop", "--", "footage", footage], cwd=ROOT
        )
    return Takes(footage)


def segments(shot):
    """(take, from, at, length) for each take in the shot, `at` seconds into the shot; the last
    one runs to the shot's end."""
    at, out = 0.0, []
    takes = shot.get("takes", [])
    for i, (take, start, *length) in enumerate(takes):
        last = i == len(takes) - 1
        span = (shot["end"] - shot["start"] - at) if last or not length else length[0]
        out.append((take, start, at, span))
        at += span
    return out


def at_time(shot, t):
    """The take, and the time into it, that plays `t` seconds into the shot."""
    for take, start, at, span in segments(shot):
        if t < at + span or (take, start, at, span) == segments(shot)[-1]:
            return take, start + (t - at)
    raise ValueError(shot)


def sheets(takes):
    """A contact sheet per take in cache/sheets/: 24 frames, evenly spaced and timestamped."""
    out = CACHE / "sheets"
    out.mkdir(parents=True, exist_ok=True)
    for take_dir in sorted(p for p in takes.root.iterdir() if p.is_dir()):
        take = take_dir.name
        n = takes.count(take)
        cols, rows, cell = 8, 3, 220
        sheet = Image.new("RGB", (cols * cell, rows * cell))
        for i in range(cols * rows):
            secs = i * (n - 1) / (cols * rows - 1) / TAKE_HZ
            im = takes.frame(take, secs).resize((cell, cell))
            ImageDraw.Draw(im).text(
                (4, 4), f"{secs:.2f}s", fill="white", stroke_width=2, stroke_fill="black"
            )
            sheet.paste(im, ((i % cols) * cell, (i // cols) * cell))
        sheet.save(out / f"{take}.png")
    print(f"contact sheets in {out}")


# --- the Blender ending ---------------------------------------------------------------------


def blender():
    """The first Blender found: $BLENDER, PATH, /Applications, then Steam."""
    for path in BLENDERS:
        if path and Path(path).exists():
            return path
    sys.exit("Blender not found: set BLENDER to its executable")


def mouth_curve(frames, bites):
    """How open the man's mouth is on every frame: shut, then wide open for each bite and
    chewing after the first."""
    t = np.arange(frames) / FPS
    mouth = np.full(frames, 0.04)
    chew = 0.06 + 0.3 * np.clip(np.sin(2 * math.pi * CHEW_HZ * t), 0, None)
    mouth[bites[0] + 3 :] = chew[bites[0] + 3 :]
    for bite in bites:
        mouth[max(0, bite - 16) : bite - 3] = np.linspace(
            0.1, 1.0, len(mouth[max(0, bite - 16) : bite - 3])
        )
        mouth[bite - 3 : bite] = 1.0
        mouth[bite : bite + 3] = 0.02
    return [round(float(m), 3) for m in mouth]


def render_eat(shot, takes):
    """Render the ending in Blender, unless frames for this config, screen, eat.py and STL exist."""
    frames = round((shot["end"] - shot["start"]) * FPS)
    bites = [round(b * FPS) for b in shot["bites"]]
    work = CACHE / "eat"
    work.mkdir(parents=True, exist_ok=True)
    screen = work / "screen.png"
    shutil.copy(takes.path(*shot["screen"]), screen)
    case = runpy.run_path(str(CASE_PARAMS))
    config = {
        "stl": str(CASE_STL),
        "screen": str(screen),
        "fps": FPS,
        "frames": frames,
        "size": [W, H],
        "samples": 24,
        "bites": bites,
        "mouth": mouth_curve(frames, bites),
        "flanges": {"back": case["BACK_FLANGE"], "front": case["FRONT_FLANGE"]},
    }
    script = HERE / "eat.py"
    digest = hashlib.sha1(script.read_bytes() + json.dumps(config, sort_keys=True).encode())
    digest.update(screen.read_bytes())
    digest.update(str(CASE_STL.stat().st_mtime_ns).encode())
    out = work / digest.hexdigest()[:12]
    if len(list(out.glob("*.png"))) < frames:
        out.mkdir(exist_ok=True)
        config["out"] = str(out)
        (out / "config.json").write_text(json.dumps(config))
        print(f"rendering the ending in Blender ({frames} frames)")
        run(
            [blender(), "-b", "--factory-startup", "-P", script, "--", out / "config.json"],
            stdout=subprocess.DEVNULL,
        )
    return out


# --- drawing ----------------------------------------------------------------------------------


def place(canvas, sprite, pivot, at, scale, angle_deg=0.0):
    """Draws `sprite` onto `canvas` with its `pivot` at `at`, scaled and turned clockwise."""
    if scale <= 0:
        return
    c, s = math.cos(math.radians(angle_deg)), math.sin(math.radians(angle_deg))
    w, h = sprite.size
    xs, ys = [], []
    for px, py in [(0, 0), (w, 0), (0, h), (w, h)]:
        dx, dy = (px - pivot[0]) * scale, (py - pivot[1]) * scale
        xs.append(at[0] + c * dx - s * dy)
        ys.append(at[1] + s * dx + c * dy)
    x0, y0 = max(0, math.floor(min(xs))), max(0, math.floor(min(ys)))
    x1, y1 = min(canvas.width, math.ceil(max(xs))), min(canvas.height, math.ceil(max(ys)))
    if x1 <= x0 or y1 <= y0:
        return
    a, b, d, e = c / scale, s / scale, -s / scale, c / scale
    coeffs = (
        a,
        b,
        pivot[0] + a * (x0 - at[0]) + b * (y0 - at[1]),
        d,
        e,
        pivot[1] + d * (x0 - at[0]) + e * (y0 - at[1]),
    )
    region = sprite.transform((x1 - x0, y1 - y0), Image.AFFINE, coeffs, resample=Image.BILINEAR)
    canvas.alpha_composite(region, (x0, y0))


def place_blurred(canvas, sprite, pivot, at, scale, angle_deg, blur, copies=9):
    """`place`, smeared toward `at` as if the camera were rushing in: `blur` 0 is sharp."""
    if blur <= 0.01:
        place(canvas, sprite, pivot, at, scale, angle_deg)
        return
    total = np.zeros((canvas.height, canvas.width, 4), np.float32)
    for i in range(copies):
        layer = Image.new("RGBA", canvas.size)
        place(layer, sprite, pivot, at, scale * (1 + 0.45 * blur * i / (copies - 1)), angle_deg)
        pixels = np.asarray(layer, np.float32)
        pixels[..., :3] *= pixels[..., 3:] / 255
        total += pixels
    total /= copies
    alpha = total[..., 3:]
    total[..., :3] = np.where(alpha > 0, total[..., :3] * 255 / np.maximum(alpha, 1e-3), 0)
    canvas.alpha_composite(Image.fromarray(np.clip(total, 0, 255).astype(np.uint8), "RGBA"))


class Device:
    """The board in its reel case, seen from the front: the game's screen inside the brown front
    flange, with the orange PWR and BOOT caps. Built at K times the panel's resolution."""

    K = 3
    CASE, CASE_LIGHT, CASE_DARK = "#6b4226", "#94653f", "#45291a"

    def __init__(self):
        k = self.K
        self.r = 233 * k
        self.rim = 30 * k
        self.size = 2 * (self.r + self.rim + 18 * k)
        self.center = (self.size / 2, self.size / 2)
        cx = cy = self.size / 2
        self.mask = Image.new("L", (2 * self.r, 2 * self.r))
        ImageDraw.Draw(self.mask).ellipse((0, 0, 2 * self.r - 1, 2 * self.r - 1), fill=255)
        ring = Image.new("RGBA", (self.size, self.size))
        draw = ImageDraw.Draw(ring)
        for deg in (60, 120):
            cap = Image.new("RGBA", (16 * k, 40 * k))
            ImageDraw.Draw(cap).rounded_rectangle(
                (0, 0, 16 * k - 1, 40 * k - 1),
                6 * k,
                fill="#ff7a1a",
                outline="#b44f08",
                width=2 * k,
            )
            a = math.radians(deg - 90)
            reach = self.r + self.rim + 3 * k
            place(
                ring,
                cap,
                (8 * k, 20 * k),
                (cx + reach * math.cos(a), cy + reach * math.sin(a)),
                1.0,
                deg - 90,
            )
        outer = self.r + self.rim
        draw.ellipse((cx - outer, cy - outer, cx + outer, cy + outer), fill=self.CASE)
        draw.ellipse(
            (cx - outer + 2 * k, cy - outer + 2 * k, cx + outer - 2 * k, cy + outer - 2 * k),
            outline=self.CASE_LIGHT,
            width=3 * k,
        )
        for groove in range(3):
            g = outer - 9 * k - groove * 5 * k
            draw.ellipse((cx - g, cy - g, cx + g, cy + g), outline=self.CASE_DARK, width=k)
        lip = self.r + 4 * k
        draw.ellipse((cx - lip, cy - lip, cx + lip, cy + lip), fill="#0b0d10")
        hole = Image.new("L", ring.size, 255)
        ImageDraw.Draw(hole).ellipse((cx - self.r, cy - self.r, cx + self.r, cy + self.r), fill=0)
        ring.putalpha(ImageChops.multiply(ring.getchannel("A"), hole))
        self.ring = ring
        shade = Image.new("L", ring.size)
        ImageDraw.Draw(shade).ellipse((cx - outer, cy - outer, cx + outer, cy + outer), fill=110)
        self.shadow = Image.new("RGBA", ring.size, (10, 10, 30, 0))
        self.shadow.putalpha(shade.filter(ImageFilter.GaussianBlur(14 * k)))
        self.sprites = OrderedDict()

    def sprite(self, screen):
        """The case around `screen`; the last 4 made are kept."""
        key = id(screen)
        if key not in self.sprites:
            out = Image.new("RGBA", (self.size, self.size))
            big = screen.resize((2 * self.r, 2 * self.r), Image.NEAREST)
            offset = int(self.size / 2 - self.r)
            out.paste(big, (offset, offset), self.mask)
            out.alpha_composite(self.ring)
            self.sprites[key] = out
            if len(self.sprites) > 4:
                self.sprites.popitem(last=False)
        return self.sprites[key]

    def screen_point(self, x, y):
        """A point on the panel (0..466) in the sprite's pixels."""
        return (
            self.size / 2 + (x - SCREEN / 2) * self.K,
            self.size / 2 + (y - SCREEN / 2) * self.K,
        )

    def draw(self, canvas, screen, at, scale, angle=0.0, pivot=None, blur=0.0, shadow=True):
        """The device turned about `pivot` (its centre by default), with a shadow unless blurred."""
        pivot = pivot or self.center
        if shadow and blur < 0.3:
            shift = (at[0] + 16, at[1] + 26)
            place(canvas, self.shadow, pivot, shift, scale, angle)
        place_blurred(canvas, self.sprite(screen), pivot, at, scale, angle, blur)


class Letters:
    """A line of text in the trailer's font, a sprite per letter so each can pop and wobble."""

    def __init__(self, font_path, text, size, fill="#ffffff", stroke=INK, max_width=None):
        font = ImageFont.truetype(str(font_path), size)
        if max_width and font.getlength(text) > max_width:
            size = int(size * max_width / font.getlength(text))
            font = ImageFont.truetype(str(font_path), size)
        stroke_w = max(2, size // 12)
        ascent, descent = font.getmetrics()
        shadow = (max(2, size // 22), max(3, size // 16))
        self.letters, x = [], 0.0
        for ch in text:
            advance = font.getlength(ch)
            if ch != " ":
                pad = stroke_w + max(shadow)
                img = Image.new("RGBA", (int(advance + 2 * pad), ascent + descent + 2 * pad))
                draw = ImageDraw.Draw(img)
                draw.text(
                    (pad + shadow[0], pad + shadow[1]),
                    ch,
                    font=font,
                    fill=(0, 0, 30, 110),
                    stroke_width=stroke_w,
                    stroke_fill=(0, 0, 30, 110),
                )
                draw.text(
                    (pad, pad), ch, font=font, fill=fill, stroke_width=stroke_w, stroke_fill=stroke
                )
                self.letters.append((img, (img.width / 2, img.height / 2), x + advance / 2))
            x += advance
        self.width = x
        self.size = size

    def draw(
        self, canvas, at, t, scale=1.0, angle=0.0, stagger=0.045, bob=0.0, jitter=None, shake=7.0
    ):
        """Letters pop in one after another from `t` = 0, bobbing by `bob` pixels; `jitter` is a
        numpy Generator for a nervous shake of up to `shake` pixels."""
        c, s = math.cos(math.radians(angle)), math.sin(math.radians(angle))
        for i, (img, pivot, x) in enumerate(self.letters):
            local = t - i * stagger
            if local <= 0:
                continue
            pop = back_out(local / 0.28)
            dx = (x - self.width / 2) * scale
            dy = bob * math.sin(t * 7 + i * 0.9) * scale
            tilt = 3 * math.sin(t * 5 + i * 1.7)
            if jitter is not None:
                dx += jitter.uniform(-1, 1) * shake * scale
                dy += jitter.uniform(-1, 1) * shake * scale
                tilt += jitter.uniform(-1, 1) * shake * 0.8
            place(
                canvas,
                img,
                pivot,
                (at[0] + c * dx - s * dy, at[1] + s * dx + c * dy),
                scale * pop,
                angle + tilt,
            )


class Backdrops:
    """Full-frame backgrounds: diagonal stripes, a sky with drifting clouds, and honeycomb."""

    def __init__(self):
        yy, xx = np.mgrid[0:H, 0:W]
        self.diagonal = (xx + yy).astype(np.int32)
        r = np.hypot((xx - W / 2) / (W / 2), (yy - H / 2) / (H / 2))
        self.vignette = (1 - 0.28 * np.clip(r - 0.35, 0, None) ** 1.5)[..., None].astype(np.float32)
        gradient = np.linspace(0, 1, H, dtype=np.float32)[:, None, None]
        sky = (
            np.array(rgb(SKY_TOP), np.float32) * (1 - gradient)
            + np.array(rgb(SKY_BOTTOM), np.float32) * gradient
        )
        self.sky_base = np.broadcast_to(sky, (H, W, 3)).astype(np.uint8)
        self.clouds = self._clouds()
        self.honeycomb = self._honeycomb()

    def _clouds(self):
        layer = Image.new("RGBA", (W * 2, H))
        draw = ImageDraw.Draw(layer)
        rng = np.random.default_rng(11)
        for _ in range(9):
            x, y = rng.uniform(0, W * 2), rng.uniform(60, H * 0.55)
            for _ in range(5):
                dx, dy, r = rng.uniform(-110, 110), rng.uniform(-30, 30), rng.uniform(50, 95)
                draw.ellipse(
                    (x + dx - r * 1.4, y + dy - r, x + dx + r * 1.4, y + dy + r),
                    fill=(255, 255, 255, 200),
                )
        return layer.filter(ImageFilter.GaussianBlur(6))

    def _honeycomb(self):
        tile = Image.new("RGB", (W + 400, H + 400), rgb("#2a1c05"))
        draw = ImageDraw.Draw(tile)
        r = 70
        for row in range(-1, (H + 400) // int(r * 1.5) + 2):
            for col in range(-1, (W + 400) // int(r * 1.732) + 2):
                cx = col * r * 1.732 + (row % 2) * r * 0.866
                cy = row * r * 1.5
                points = [
                    (
                        cx + r * math.cos(math.radians(60 * k + 30)),
                        cy + r * math.sin(math.radians(60 * k + 30)),
                    )
                    for k in range(6)
                ]
                draw.polygon(points, outline=rgb("#7a5208"), width=6)
        return tile

    def stripes(self, color, t, speed=50, width=70, lighten=0.12):
        base = np.array(rgb(color), np.float32)
        light = base + (255 - base) * lighten
        mask = ((self.diagonal + int(t * speed)) % (2 * width)) < width
        pixels = np.where(mask[..., None], light, base) * self.vignette
        return Image.fromarray(pixels.astype(np.uint8)).convert("RGBA")

    def sky(self, t):
        canvas = Image.fromarray(self.sky_base).convert("RGBA")
        shift = int(t * 25) % W
        canvas.alpha_composite(self.clouds.crop((shift, 0, shift + W, H)))
        return canvas

    def honey(self, t):
        x, y = int(t * 90) % 242, int(t * 60) % 210
        return self.honeycomb.crop((x, y, x + W, y + H)).convert("RGBA")


def bee_sprite():
    """A little pixel bee for the BEES card, drawn like the game's."""
    rows = [
        "....ww..ww....",
        "...wwww.www...",
        "....ww..ww....",
        "..yykkyykkyy..",
        ".yyykkyykkyyk.",
        "kyyykkyykkyykk",
        ".yyykkyykkyyk.",
        "..yykkyykkyy..",
    ]
    colors = {"w": (235, 245, 255, 230), "y": (255, 200, 30, 255), "k": (25, 20, 10, 255)}
    img = Image.new("RGBA", (len(rows[0]), len(rows)))
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch in colors:
                img.putpixel((x, y), colors[ch])
    return img.resize((img.width * 7, img.height * 7), Image.NEAREST)


# --- the shots --------------------------------------------------------------------------------


class Trailer:
    """Draws any frame: `frame(t)` finds the shot and calls the method named after its kind."""

    def __init__(self, takes, font, eat_frames):
        self.takes = takes
        self.font = font
        self.eat_frames = eat_frames
        self.device = Device()
        self.back = Backdrops()
        self.bee = bee_sprite()
        self.words = {}
        self.frozen = {}
        self.beats = [s for s in edit.SHOTS if s["kind"] == "beat"]

    def text(self, text, size, **kwargs):
        key = (text, size, tuple(sorted(kwargs.items())))
        if key not in self.words:
            self.words[key] = Letters(self.font, text, size, **kwargs)
        return self.words[key]

    def frame(self, t):
        for index, shot in enumerate(edit.SHOTS):
            if shot["start"] <= t < shot["end"] or index == len(edit.SHOTS) - 1:
                draw = getattr(self, shot["kind"])
                return draw(shot, t - shot["start"], index).convert("RGB")
        raise ValueError(t)

    def screen(self, shot, t):
        return self.takes.frame(*at_time(shot, t))

    def opening(self, shot, t, index):
        """The title beside the player at their house, then the dive into the player."""
        canvas = self.back.sky(t)
        length = shot["end"] - shot["start"]
        dive = clamp(
            (t - (shot["dive"] - shot["start"])) / (length - (shot["dive"] - shot["start"]))
        )
        rush = dive**2.2
        base_at, base_scale, angle = (1265, 560), 0.6 * (1 + 0.03 * t), -3 + 1.2 * math.sin(t * 0.9)
        x, y = self.takes.focus(shot["takes"][0][0])
        focus = self.device.screen_point(x, y - 14)
        c, s = math.cos(math.radians(angle)), math.sin(math.radians(angle))
        dx, dy = (
            (focus[0] - self.device.center[0]) * base_scale,
            (focus[1] - self.device.center[1]) * base_scale,
        )
        start = (base_at[0] + c * dx - s * dy, base_at[1] + s * dx + c * dy)
        at = (
            start[0] + (W / 2 - start[0]) * smooth(dive),
            start[1] + (H / 2 - start[1]) * smooth(dive),
        )
        scale = base_scale * math.exp(math.log(16) * rush)
        self.device.draw(
            canvas, self.screen(shot, t), at, scale, angle * (1 - dive), focus, blur=rush * 1.4
        )
        for i, line in enumerate(shot["title"]):
            letters = self.text(line, 190, max_width=820)
            at = (560 - rush * 1400, 390 + i * 230 - rush * 300)
            letters.draw(
                canvas, at, t - 0.2 - i * 0.35, scale=1 + 2.5 * rush, angle=-5 + i * 3, bob=5
            )
        return canvas

    def beat(self, shot, t, index):
        """A gameplay shot: the device on stripes beside its label, alternating sides."""
        n = self.beats.index(shot)
        canvas = self.back.stripes(shot["color"], t + n * 3)
        length = shot["end"] - shot["start"]
        u = t / length
        left = n % 2 == 0
        zoom = 0.93 + 0.13 * smooth(u) if left else 1.1 - 0.13 * smooth(u)
        punch = 1 + 0.14 * math.exp(-t / 0.07)
        sign = 1 if left else -1
        angle = sign * (5 + 1.4 * math.sin(t * 2.2))
        at = (630 if left else 1290, 560 + 12 * math.sin(t * 2.6))
        blur = clamp(1 - t / 0.22) * 1.2 if n == 0 else 0.0
        self.device.draw(canvas, self.screen(shot, t), at, 0.54 * zoom * punch, angle, blur=blur)
        label = self.text(shot["label"], 250, max_width=660)
        label.draw(
            canvas,
            (1450 if left else 470, 545),
            t - 0.05,
            scale=1 + 0.05 * u,
            angle=-sign * 7,
            bob=6,
        )
        return canvas

    def scratch(self, shot, t, index):
        """The last frame of the shot before, frozen, draining to grey and fading to black."""
        previous = edit.SHOTS[index - 1]
        key = id(previous)
        if key not in self.frozen:
            self.frozen[key] = self.frame(previous["end"] - 1 / FPS).convert("RGB")
        frozen = self.frozen[key]
        u = t / (shot["end"] - shot["start"])
        canvas = Image.new("RGBA", (W, H), (0, 0, 0, 255))
        grey = Image.blend(frozen, frozen.convert("L").convert("RGB"), clamp(u * 2.5))
        place(
            canvas,
            grey.convert("RGBA"),
            (W / 2, H / 2),
            (W / 2, H / 2),
            1 + 0.12 * u,
            4 * math.sin(u * 9) * u,
        )
        return Image.blend(canvas.convert("RGB"), Image.new("RGB", (W, H)), clamp(u * 1.4) ** 1.5)

    def chaos(self, shot, t, index):
        """BEES, SKATEBOARDS, SPIDERS: shaking on fast backdrops; a `pounce` comes at you."""
        if shot.get("honey"):
            canvas = self.back.honey(t * 2)
        else:
            canvas = self.back.stripes(shot["color"], t * 4, width=55, lighten=0.18)
        jitter = np.random.default_rng(int((shot["start"] + t) * FPS))
        pulse = 1 + 0.06 * math.exp(-(t % 0.32) / 0.06)
        has_label = "label" in shot
        at = [620 if has_label else W / 2, 560]
        scale = (0.52 if has_label else 0.58) * pulse
        angle = 8 * math.sin(t * 4.3) + jitter.uniform(-1.5, 1.5)
        shake = 9.0
        pounced = "pounce" in shot and t >= shot["pounce"]
        if pounced:
            p = clamp((t - shot["pounce"]) / 0.28)
            scale *= 1 + 1.7 * p**2
            at = [at[0] + (W / 2 - at[0]) * smooth(p), at[1] + (H / 2 - at[1]) * smooth(p)]
            shake = 9 + 30 * math.exp(-(t - shot["pounce"]) / 0.35)
            angle *= 1 - p
        at = (at[0] + jitter.uniform(-1, 1) * shake, at[1] + jitter.uniform(-1, 1) * shake)
        self.device.draw(canvas, self.screen(shot, t), at, scale, angle)
        if shot.get("honey"):
            for i in range(6):
                phase = t * (2.2 + 0.3 * i) + i * math.tau / 6
                x = 1450 + math.cos(phase) * (400 + 30 * math.sin(i))
                y = 540 + math.sin(phase * 1.3) * 260
                place(canvas, self.bee, (49, 28), (x, y), 1.0, 12 * math.sin(phase * 5))
        if has_label and not pounced:
            colors = {"fill": "#ffd21f", "stroke": "#1a1000"} if shot.get("honey") else {}
            label = self.text(shot["label"], 190, max_width=760, **colors)
            label.draw(
                canvas,
                (1450, 540),
                t - 0.05,
                angle=-8 + 3 * math.sin(t * 3),
                stagger=0.03,
                jitter=jitter,
                shake=2.5,
            )
        if pounced:
            flash = math.exp(-(t - shot["pounce"]) / 0.12)
            canvas = Image.blend(
                canvas.convert("RGB"), Image.new("RGB", (W, H), (255, 30, 30)), 0.45 * flash
            )
        return canvas

    def eat(self, shot, t, index):
        """The Blender ending, with YOU CAN EAT IT popping in a word at a time."""
        frames = sorted(self.eat_frames.glob("[0-9]*.png"))
        canvas = Image.open(frames[min(round(t * FPS), len(frames) - 1)]).convert("RGBA")
        if canvas.size != (W, H):
            canvas = canvas.resize((W, H))
        words = shot["words"]
        spacing = 70
        sizes = [self.text(word, 190) for word in words]
        total = sum(w.width for w in sizes) + spacing * (len(words) - 1)
        x = W / 2 - total / 2
        for i, word in enumerate(sizes):
            appear = shot["words_at"] + i * 0.3
            word.draw(
                canvas,
                (x + word.width / 2, 920),
                t - appear,
                angle=(-4 if i % 2 else 3),
                stagger=0.03,
                bob=4,
            )
            x += word.width + spacing
        return canvas

    def guten(self, shot, t, index):
        """The spinning fish beside the message, then the title tag in the corner."""
        canvas = self.back.sky(t + 40)
        u = t / (shot["end"] - shot["start"])
        lines = shot.get("message", [])
        at = (680, H / 2) if lines else (W / 2, H / 2)
        scale = (0.54 if lines else 0.6) * (1 + 0.05 * u)
        self.device.draw(canvas, self.screen(shot, t), at, scale, 0.0)
        for i, line in enumerate(lines):
            gold = i == len(lines) - 1
            letters = self.text(line, 200, fill="#ffd21f" if gold else "#ffffff", max_width=700)
            letters.draw(
                canvas,
                (1440, H / 2 + (i - (len(lines) - 1) / 2) * 215),
                t - shot["message_at"] - i * 0.35,
                angle=(-5, 3, -3)[i % 3],
                bob=6,
            )
        if t > 1.8:
            tag = self.text(shot["tag"], 64)
            tag.draw(
                canvas, (W - 60 - tag.width / 2, H - 70), t - 1.8, stagger=0.03, angle=-3, bob=2
            )
        return canvas


# --- sound ------------------------------------------------------------------------------------


def mix(takes, music_files):
    """cache/mix.wav: the music cues, the scratch, whoosh and crunches, and the takes' sounds."""
    out = np.zeros((int(edit.LENGTH * SR) + SR, 2), np.float32)

    def add(clip, at, gain=1.0, fade_out=0.005, fade_in=0.005):
        if clip.ndim == 1:
            clip = np.stack([clip, clip], axis=1)
        start = int(at * SR)
        clip = clip[: max(0, len(out) - start)].copy()
        rise = min(len(clip), int(fade_in * SR))
        if rise:
            clip[:rise] *= np.linspace(0, 1, rise, dtype=np.float32)[:, None]
        fall = min(len(clip), int(fade_out * SR))
        if fall:
            clip[-fall:] *= np.linspace(1, 0, fall, dtype=np.float32)[:, None]
        out[start : start + len(clip)] += clip * gain

    music = {name: load_audio(path) for name, path in music_files.items()}
    for cue in edit.SCORE:
        track = music[cue["track"]]
        offset = cue.get("offset")
        offset = first_sound(track) if offset is None else offset
        piece = track[int(offset * SR) : int((offset + cue["end"] - cue["start"]) * SR)].copy()
        if "level" in cue:
            piece *= level(cue["level"], cue["start"], len(piece))[:, None]
        add(
            piece,
            cue["start"],
            db(edit.MUSIC[cue["track"]]["gain_db"]),
            fade_in=cue.get("fade_in", 0.005),
            fade_out=cue.get("fade_out", 0.005),
        )
    for shot in edit.SHOTS:
        if shot["kind"] == "scratch":
            cue = edit.SCORE[0]
            add(
                scratch(music[cue["track"]], cue["offset"] + shot["start"] - cue["start"]),
                shot["start"],
                db(edit.MUSIC[cue["track"]]["gain_db"] + 2),
            )
        if shot["kind"] == "opening":
            add(whoosh(shot["end"] - shot["dive"]), shot["dive"], 0.35)
        if shot["kind"] == "eat":
            bites = shot["bites"]
            for i, bite in enumerate(bites):
                add(crunch(i), shot["start"] + bite - 0.02, 1.8)
                add(thump(), shot["start"] + bite - 0.02, 1.2)
            # A crunch each time the chewing mouth shuts (see mouth_curve).
            for k in range(int((shot["end"] - shot["start"]) * CHEW_HZ)):
                shuts = (k + 0.5) / CHEW_HZ
                biting = any(bite - 0.6 < shuts < bite + 0.15 for bite in bites)
                if shuts > bites[0] + 0.15 and not biting:
                    add(crunch(10 + k), shot["start"] + shuts, 0.45)
        for take, start, at, span in segments(shot):
            if take in shot.get("mute", []):
                continue
            sound = takes.audio(take)
            end = ring_out(sound, int((start + span) * SR))
            add(
                sound[int(start * SR) : end],
                shot["start"] + at,
                db(edit.GAME_SOUNDS_DB),
                fade_out=0.03,
            )
    out = out[: int(edit.LENGTH * SR)]
    out *= 0.89 / (np.abs(out).max() + 1e-9)
    wav = CACHE / "mix.wav"
    pcm = (np.clip(out, -1, 1) * 32767).astype("<i2")
    with open(wav, "wb") as f:
        data = pcm.tobytes()
        f.write(b"RIFF" + (36 + len(data)).to_bytes(4, "little") + b"WAVEfmt ")
        f.write((16).to_bytes(4, "little") + (1).to_bytes(2, "little") + (2).to_bytes(2, "little"))
        f.write(
            SR.to_bytes(4, "little") + (SR * 4).to_bytes(4, "little") + (4).to_bytes(2, "little")
        )
        f.write((16).to_bytes(2, "little") + b"data" + len(data).to_bytes(4, "little") + data)
    return wav


# --- putting it together ----------------------------------------------------------------------


def encode(trailer, audio, preview):
    """Pipe every frame and the mix into ffmpeg for the MP4 (or the half-size preview)."""
    OUT.mkdir(exist_ok=True)
    fps = 15 if preview else FPS
    size = (W // 2, H // 2) if preview else (W, H)
    name = "fishing_minigame_trailer_preview.mp4" if preview else "fishing_minigame_trailer.mp4"
    path = OUT / name
    cmd = [
        "ffmpeg",
        "-v",
        "error",
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgb24",
        "-s",
        f"{size[0]}x{size[1]}",
        "-r",
        str(fps),
        "-i",
        "-",
        "-i",
        str(audio),
        "-c:v",
        "libx264",
        "-preset",
        "veryfast" if preview else "slow",
        "-crf",
        "23" if preview else "17",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-af",
        "loudnorm=I=-15:TP=-1.5:LRA=11",
        "-ar",
        str(SR),
        "-movflags",
        "+faststart",
        "-shortest",
        str(path),
    ]
    encoder = subprocess.Popen(cmd, stdin=subprocess.PIPE)
    frames = round(edit.LENGTH * fps)
    for i in range(frames):
        image = trailer.frame(i / fps)
        if preview:
            image = image.resize(size)
        encoder.stdin.write(image.tobytes())
        if i % fps == 0:
            print(f"\r  {i / fps:5.1f} / {edit.LENGTH:.0f} s", end="", flush=True)
    encoder.stdin.close()
    if encoder.wait():
        sys.exit("ffmpeg failed")
    print(f"\nwrote {path}")


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--refilm", action="store_true", help="film the takes again")
    parser.add_argument(
        "--sheets", action="store_true", help="contact sheets of the takes, then stop"
    )
    parser.add_argument(
        "--still", type=float, action="append", help="write the frame at these seconds, then stop"
    )
    parser.add_argument("--preview", action="store_true", help="half size, 15 fps")
    args = parser.parse_args()

    font, music = fetch_assets()
    takes = film(args.refilm)
    if args.sheets:
        sheets(takes)
        return
    ending = next(shot for shot in edit.SHOTS if shot["kind"] == "eat")
    trailer = Trailer(takes, font, render_eat(ending, takes))
    if args.still:
        OUT.mkdir(exist_ok=True)
        for secs in args.still:
            path = OUT / f"still-{secs:05.2f}.png"
            trailer.frame(secs).save(path)
            print(f"wrote {path}")
        return
    encode(trailer, mix(takes, music), args.preview)


if __name__ == "__main__":
    main()
