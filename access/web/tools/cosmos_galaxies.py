#!/usr/bin/env python3
"""The near galaxies of the cosmos between pages (public/cosmos.js), and the atlas of small ones for its crowd, drawn
once by hand and kept as images (public/cosmos/*.webp).

Each is its own kind, after a galaxy the sky really holds: the Whirlpool with its companion, a barred spiral, the
flocculent Sunflower, the Sombrero with its ring of dust, and Hoag's ring with the small ring seen through its gap.
Light is added as light adds: a bulge of old yellow stars, an exponential disc, arms gathered in clumps, knots of
young blue stars and pink nebulae, resolved stars; dust takes light away and reddens what it lets through; and the
whole is compressed as the eye compresses it, with a little bloom, so a bright core glows and never burns to a blot.

Usage (from access/web): python3 tools/cosmos_galaxies.py public/cosmos [SCALE [NAMES]]
  a SCALE below 1 renders small previews; NAMES picks some, comma-separated (whirlpool,barred,...).
"""
import math
import os
import sys

import numpy as np
from PIL import Image

OUT = sys.argv[1] if len(sys.argv) > 1 else '.'
SCALE = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0
SS = 2  # every image is drawn at twice its size and brought down, so its edges and its stars are smooth

WARM = np.array([1.0, .82, .56])
GOLD = np.array([1.0, .88, .66])
WHITE = np.array([1.0, .96, .9])
BLUE = np.array([.58, .73, 1.0])
PALE = np.array([.78, .86, 1.0])
PINK = np.array([1.0, .42, .56])
DUST = np.array([.55, .7, .9])  # dust takes blue first: what passes it reddens


def smoothstep(a, b, x):
    t = np.clip((x - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)


def blur(img, sigma):
    """A gaussian blur by the Fourier transform; the images are dark at their edges, so the wrap is unseen."""
    if sigma <= 0:
        return img
    h, w = img.shape[:2]
    fy = np.fft.fftfreq(h)[:, None]
    fx = np.fft.rfftfreq(w)[None, :]
    k = np.exp(-2 * (np.pi * sigma) ** 2 * (fx ** 2 + fy ** 2))
    if img.ndim == 2:
        return np.fft.irfft2(np.fft.rfft2(img) * k, s=(h, w))
    return np.stack([np.fft.irfft2(np.fft.rfft2(img[..., c]) * k, s=(h, w)) for c in range(img.shape[2])], -1)


def noise(shape, sigma, seed, contrast=1.6):
    """Smooth noise in 0..1 with a grain of `sigma` pixels; it wraps, so it can be read in polar coordinates."""
    n = blur(np.random.default_rng(seed).standard_normal(shape), sigma)
    n = (n - n.mean()) / (n.std() + 1e-9)
    return 1 / (1 + np.exp(-n * contrast))


def sample(texture, u, v):
    """Read a wrapping texture at fractional pixel coordinates, bilinearly."""
    h, w = texture.shape
    u = np.mod(u, w)
    v = np.mod(v, h)
    x0 = np.floor(u).astype(int)
    y0 = np.floor(v).astype(int)
    ax = u - x0
    ay = v - y0
    x1 = (x0 + 1) % w
    y1 = (y0 + 1) % h
    return (texture[y0, x0] * (1 - ax) * (1 - ay) + texture[y0, x1] * ax * (1 - ay)
            + texture[y1, x0] * (1 - ax) * ay + texture[y1, x1] * ax * ay)


class Frame:
    """A plate to draw a galaxy on: x runs from -1 to 1 across it, y the same per unit, downwards."""

    def __init__(self, width, height, seed):
        self.w = max(8, int(width * SCALE)) * SS
        self.h = max(8, int(height * SCALE)) * SS
        self.rng = np.random.default_rng(seed)
        self.seed = seed
        self.light = np.zeros((self.h, self.w, 3))
        xs = (np.arange(self.w) + .5) / self.w * 2 - 1
        ys = ((np.arange(self.h) + .5) / self.h * 2 - 1) * self.h / self.w
        self.x, self.y = np.meshgrid(xs, ys)

    def add(self, intensity, color):
        color = np.asarray(color, dtype=float)
        if color.ndim == 1:
            self.light += intensity[..., None] * color
        else:
            self.light += intensity[..., None] * color

    def noise(self, grain, seed, contrast=1.6):
        return noise((self.h, self.w), grain * self.w, self.seed * 31 + seed, contrast)

    def stars(self, xs, ys, peaks, colors, size=.0012, glow=0.0):
        """Point lights at (xs, ys) with the given peak brightness; `size` is their radius in frame units."""
        xs, ys, peaks = np.asarray(xs), np.asarray(ys), np.asarray(peaks)
        colors = np.broadcast_to(np.asarray(colors, dtype=float), (len(xs), 3))
        img = np.zeros_like(self.light)
        fx = (xs + 1) / 2 * self.w - .5
        fy = (ys * self.w / self.h + 1) / 2 * self.h - .5
        ix = np.floor(fx).astype(int)
        iy = np.floor(fy).astype(int)
        ax = fx - ix
        ay = fy - iy
        sigma = max(.6, size * self.w / 2)
        weight = peaks * 2 * math.pi * sigma ** 2
        for dx, dy, part in ((0, 0, (1 - ax) * (1 - ay)), (1, 0, ax * (1 - ay)), (0, 1, (1 - ax) * ay), (1, 1, ax * ay)):
            X = ix + dx
            Y = iy + dy
            ok = (X >= 0) & (X < self.w) & (Y >= 0) & (Y < self.h)
            for c in range(3):
                np.add.at(img[..., c], (Y[ok], X[ok]), (weight * part * colors[:, c])[ok])
        self.light += blur(img, sigma)
        if glow:
            self.light += blur(img, sigma * 5) * glow

    def save(self, name, exposure, bloom=.14, halo=.03, round=True):
        light = self.light + blur(self.light, .012 * self.w) * bloom + blur(self.light, .06 * self.w) * halo
        # No light reaches the plate's edge: whatever stands near it fades out before it, so no edge is ever seen. A
        # galaxy that turns on the page fades within a circle, so not even the faintest corner of its plate turns with it.
        aspect = self.h / self.w
        if round:
            light = light * smoothstep(1, .7, np.hypot(self.x, self.y / aspect))[..., None]
        else:
            light = light * (smoothstep(1, .9, np.abs(self.x)) * smoothstep(aspect, aspect * .86, np.abs(self.y)))[..., None]
        t = 1 - np.exp(-np.maximum(light, 0) * exposure)
        a = t.max(-1)
        # Brought down to its size premultiplied, then the colour is taken back out of the light.
        h, w = self.h // SS, self.w // SS
        t = t.reshape(h, SS, w, SS, 3).mean((1, 3))
        a = a.reshape(h, SS, w, SS).mean((1, 3))
        rgb = np.where(a[..., None] > 1e-4, t / np.maximum(a[..., None], 1e-4), 0)
        img = np.concatenate([np.clip(rgb, 0, 1), np.clip(a, 0, 1)[..., None]], -1)
        Image.fromarray((img * 255 + .5).astype(np.uint8), 'RGBA').save(os.path.join(OUT, name + '.webp'), 'WEBP',
                                                                          quality=88, alpha_quality=90, method=6)
        return name


def arm_points(rng, count, radii, k, phase, arms, spread, center, scale, angle=0.0):
    """Points along the arms of a logarithmic spiral: where young stars and nebulae gather."""
    r = rng.uniform(radii[0], radii[1], count)
    m = rng.integers(0, arms, count)
    theta = k * np.log(r) + (2 * math.pi * m - phase) / arms + rng.normal(0, spread, count)
    r = r * (1 + rng.normal(0, spread * .4, count))
    return center[0] + np.cos(theta + angle) * r * scale, center[1] + np.sin(theta + angle) * r * scale, r


def clusters(rng, centers_x, centers_y, per, spread):
    """Knots of young stars and nebulae come in families: a few round each birthplace."""
    counts = rng.integers(per[0], per[1] + 1, len(centers_x))
    xs = np.repeat(centers_x, counts) + rng.normal(0, spread, counts.sum())
    ys = np.repeat(centers_y, counts) + rng.normal(0, spread, counts.sum())
    return xs, ys


def whirlpool():
    """The Whirlpool (M51): a grand design of two arms, pink with nebulae, and its companion at the end of one."""
    f = Frame(1024, 1024, 51)
    cx, cy, R = -.1, .08, .6
    X, Y = f.x - cx, f.y - cy
    r = np.hypot(X, Y) / R
    th = np.arctan2(Y, X)
    k, phase = 1 / math.tan(math.radians(18)), .4
    phi = 2 * (th - k * np.log(r + 1e-3)) + phase
    c = (np.cos(phi) + 1) / 2
    fine, mid, large = f.noise(.007, 1), f.noise(.022, 2), f.noise(.07, 3)
    env = smoothstep(.06, .2, r) * (1 - smoothstep(.82, 1.18, r))
    arms = env * (c ** 3.4 * (.3 + 1.4 * fine) * (.55 + .9 * mid) + c ** 1.3 * .55 * (.6 + .8 * mid))
    lane = ((np.cos(phi + .55) + 1) / 2) ** 10 * smoothstep(.08, .24, r) * (1 - smoothstep(.9, 1.12, r)) * (.6 + .6 * mid)
    warmth = 1 - smoothstep(.1, .55, r)
    color = BLUE[None, None] * (1 - warmth[..., None]) + GOLD[None, None] * warmth[..., None]
    disc = np.exp(-r / .38) * (.36 + 2.0 * arms) * (.85 + .3 * large) + np.exp(-r / .55) * .045
    light = disc[..., None] * color + ((np.exp(-(r / .06) ** .7 * 2) * 2.4 + np.exp(-r / .15) * .45))[..., None] * WARM
    f.light += light * (1 - np.clip(.85 * lane, 0, .9)[..., None] * DUST)
    # The companion (NGC 5195) where the arm ends: a yellow elliptical with a lane of the arm's dust across it.
    ex = cx + math.cos(k * math.log(1.22) - phase / 2) * 1.22 * R
    ey = cy + math.sin(k * math.log(1.22) - phase / 2) * 1.22 * R
    d = np.hypot(f.x - ex, (f.y - ey) / .82)
    companion = np.exp(-(d / .035) ** .7 * 2) * 2 + np.exp(-d / .08) * .45 + np.exp(-d / .18) * .08
    across = np.exp(-(((f.x - ex) * math.cos(.9) + (f.y - ey) * math.sin(.9)) / .03) ** 2) * np.exp(-(d / .1) ** 2) * .45
    f.light += (companion[..., None] * np.array([1, .78, .52])) * (1 - across[..., None] * DUST)
    # The bridge of stars it draws out of the arm.
    sx = cx + math.cos(k * math.log(.92) - phase / 2) * .92 * R
    sy = cy + math.sin(k * math.log(.92) - phase / 2) * .92 * R
    t = np.clip(((f.x - sx) * (ex - sx) + (f.y - sy) * (ey - sy)) / ((ex - sx) ** 2 + (ey - sy) ** 2), 0, 1)
    gap = np.hypot(f.x - (sx + t * (ex - sx)), f.y - (sy + t * (ey - sy)))
    f.add(np.exp(-(gap / .07) ** 2) * .3 * (.35 + mid) * (1 - t * .55), PALE * .7 + GOLD * .3)
    rng = f.rng
    x, y, rr = arm_points(rng, 190, (.22, .98), k, phase, 2, .085, (cx, cy), R)
    kx, ky = clusters(rng, x, y, (3, 8), .006)
    f.stars(kx, ky, rng.uniform(.5, 1.8, len(kx)), np.where(rng.uniform(0, 1, len(kx))[:, None] < .7, PINK, PALE), size=.002, glow=.3)
    x, y, rr = arm_points(rng, 3200, (.15, 1.02), k, phase, 2, .1, (cx, cy), R)
    f.stars(x, y, rng.uniform(.2, .8, len(x)), PALE, size=.0012)
    rr = rng.exponential(.28, 2200)
    tt = rng.uniform(0, 2 * math.pi, 2200)
    f.stars(cx + np.cos(tt) * rr * R, cy + np.sin(tt) * rr * R, rng.uniform(.08, .35, 2200), GOLD, size=.001)
    return f.save('whirlpool', exposure=1.8)


def barred():
    """A barred spiral (after NGC 1300): a golden bar with brighter ends, and two arms that leave from them."""
    f = Frame(1024, 1024, 1300)
    R, beta = .8, .35
    U = (f.x * math.cos(beta) + f.y * math.sin(beta)) / R
    V = (-f.x * math.sin(beta) + f.y * math.cos(beta)) / R
    r = np.hypot(U, V)
    th = np.arctan2(V, U)
    k, r0 = 1 / math.tan(math.radians(20)), .34
    phi = 2 * (th - k * np.log(np.maximum(r, 1e-3) / r0))
    c = (np.cos(phi) + 1) / 2
    fine, mid = f.noise(.007, 1), f.noise(.03, 2)
    env = smoothstep(.31, .4, r) * (1 - smoothstep(.84, 1.08, r))
    arms = env * (c ** 6 * (.3 + 1.4 * fine) * (.55 + .9 * mid) + c ** 2.2 * .2 * (.6 + .8 * mid))
    lane = ((np.cos(phi + .5) + 1) / 2) ** 14 * env * (.6 + .6 * mid)
    bar = np.exp(-(U / .34) ** 4 - (V / .058) ** 2) * .95 + np.exp(-((np.abs(U) - .29) / .05) ** 2 - (V / .05) ** 2) * .3
    bar_lane = np.exp(-((V - .03 * np.sign(U)) / .014) ** 2) * np.exp(-(U / .3) ** 4) * smoothstep(.08, .18, np.abs(U)) * .3
    ring = np.exp(-((r - .35) / .05) ** 2) * .05 * (.3 + 1.2 * fine)
    warmth = 1 - smoothstep(.3, .62, r)
    color = BLUE[None, None] * (1 - warmth[..., None]) + GOLD[None, None] * warmth[..., None]
    light = ((np.exp(-r / .4) * (.1 + 2.5 * arms) + ring))[..., None] * color
    light = light * (1 - np.clip(.8 * lane, 0, .9)[..., None] * DUST)
    light += ((bar + np.exp(-(r / .05) ** .8 * 1.8) * 2.2) * (1 - bar_lane))[..., None] * WARM
    f.light += light
    rng = f.rng
    rr = rng.uniform(.42, .92, 150)
    m = rng.integers(0, 2, 150)
    tt = k * np.log(rr / r0) + math.pi * m + rng.normal(0, .035, 150) + beta
    kx, ky = clusters(rng, np.cos(tt) * rr * R, np.sin(tt) * rr * R, (3, 7), .006)
    f.stars(kx, ky, rng.uniform(.5, 1.6, len(kx)), np.where(rng.uniform(0, 1, len(kx))[:, None] < .6, PINK, PALE), size=.002, glow=.3)
    rr = rng.uniform(.4, 1.02, 2600)
    m = rng.integers(0, 2, 2600)
    tt = k * np.log(rr / r0) + math.pi * m + rng.normal(0, .09, 2600) + beta
    f.stars(np.cos(tt) * rr * R, np.sin(tt) * rr * R, rng.uniform(.2, .8, 2600), PALE, size=.0012)
    return f.save('barred', exposure=1.9)


def sunflower():
    """The Sunflower (M63): no grand design, only a wealth of short arms, golden within and blue at the rim."""
    f = Frame(1024, 1024, 63)
    R = .8
    r = np.hypot(f.x, f.y) / R
    th = np.arctan2(f.y, f.x)
    k = 1 / math.tan(math.radians(20))
    s = th - k * np.log(np.maximum(r, 1e-3))
    t = np.log(np.maximum(r, 1e-3))
    texture = noise((512, 512), 7, 6301, 2.0)
    dust = noise((512, 512), 2.5, 6302, 2.4)
    u = s / (2 * math.pi) * 512
    fluff = f.noise(.006, 4) * .6 + f.noise(.016, 5) * .4
    flocks = 1 / (1 + np.exp(-(sample(texture, u, t * 70) - .5) * 9)) * (.35 + 1.1 * fluff)
    filaments = np.clip(sample(dust, u * 2 + 17, t * 90 + 9) - .62, 0, None) * 2.2
    env = smoothstep(.05, .2, r) * (1 - smoothstep(.72, 1.05, r))
    warmth = 1 - smoothstep(.22, .78, r)
    color = np.array([.7, .8, 1.0])[None, None] * (1 - warmth[..., None]) + GOLD[None, None] * warmth[..., None]
    light = (np.exp(-r / .33) * (.42 + 1.15 * flocks * env))[..., None] * color
    light = light * (1 - np.clip(.6 * filaments * env, 0, .85)[..., None] * DUST)
    light += (np.exp(-(r / .07) ** .75 * 1.8) * 2.1 + np.exp(-r / .19) * .3)[..., None] * WARM
    f.light += light
    rng = f.rng
    rr = rng.uniform(.12, 1.0, 6000)
    tt = rng.uniform(0, 2 * math.pi, 6000)
    uu = (tt - k * np.log(rr)) / (2 * math.pi) * 512
    on = 1 / (1 + np.exp(-(sample(texture, uu, np.log(rr) * 70) - .5) * 9))
    keep = rng.uniform(0, 1, 6000) < on
    colors = np.where((rr < .45)[:, None], GOLD[None], PALE[None])
    f.stars(np.cos(tt[keep]) * rr[keep] * R, np.sin(tt[keep]) * rr[keep] * R, rng.uniform(.2, .7, keep.sum()), colors[keep], size=.0012)
    return f.save('sunflower', exposure=1.45)


def sombrero():
    """The Sombrero (M104): a great golden bulge, and a ring of dust seen almost edge on crossing in front of it."""
    f = Frame(1024, 512, 104)
    u, v = f.x / .9, f.y / .9
    d = np.hypot(u, v / .6)
    bulge = np.exp(-(d / .045) ** .55 * 2.4) * 2.8 + np.exp(-(d / .2) ** 1.15) * .85
    q = .12
    rho = np.hypot(u, v / q)
    plane = np.exp(-(np.abs(v) / (q * .6)) ** 2)
    n = f.noise(.008, 3)
    ring = (np.exp(-((rho - .86) / .13) ** 2) * .55 * (.6 + .6 * n) + np.exp(-((rho - .5) / .3) ** 2) * .1) * (rho < 1.1)
    near = v > 0
    lane = np.exp(-((rho - .8) / .1) ** 2) * np.where(near, 1.0, .35) * (.75 + .4 * n)
    absorb = np.clip(.95 * lane, 0, .95)
    light = bulge[..., None] * WARM + ring[..., None] * np.array([1, .9, .78])
    f.light += light * (1 - absorb[..., None] * DUST)
    rng = f.rng
    # Globular clusters round the bulge, a swarm of old suns.
    gr = rng.pareto(2.0, 320) * .12 + .16
    gt = rng.uniform(0, 2 * math.pi, 320)
    f.stars(np.cos(gt) * gr * .9, np.sin(gt) * gr * .9 * .66, rng.uniform(.2, .6, 320), GOLD, size=.0015)
    return f.save('sombrero', exposure=1.35, halo=.015, round=False)


def hoag():
    """Hoag's object: a yellow core, a dark gap, a near perfect ring of blue stars, and another ring seen through the gap."""
    f = Frame(768, 768, 2015)
    r = np.hypot(f.x, f.y)
    fine, mid = f.noise(.01, 1), f.noise(.04, 2)
    f.add(np.exp(-(r / .06) ** .7 * 1.9) * 2.4 + np.exp(-r / .11) * .35, np.array([1, .78, .48]))
    f.add(np.exp(-((r - .64) / (.045 + .015 * mid)) ** 2) * (.25 + 1.4 * fine), np.array([.55, .72, 1.0]))
    f.add(np.exp(-((r - .62) / .2) ** 2) * .05, PALE)
    # Through the gap, far behind: a small reddish ring of its own, a little turned.
    bx, by = .26, -.2
    du, dv = (f.x - bx) * math.cos(.5) + (f.y - by) * math.sin(.5), -(f.x - bx) * math.sin(.5) + (f.y - by) * math.cos(.5)
    br = np.hypot(du, dv / .7)
    f.add(np.exp(-(br / .01) ** 2) * .5 + np.exp(-((br - .038) / .01) ** 2) * .16, np.array([1, .6, .45]))
    rng = f.rng
    rr = rng.normal(.64, .028, 1500)
    tt = rng.uniform(0, 2 * math.pi, 1500)
    f.stars(np.cos(tt) * rr, np.sin(tt) * rr, rng.uniform(.3, 1.1, 1500), np.where(rng.uniform(0, 1, 1500)[:, None] < .85, PALE, PINK), size=.0014, glow=.15)
    return f.save('hoag', exposure=1.8)


CELLS = 5


def atlas():
    """Twenty-five small galaxies, each drawn as carefully as the near ones, for the middle of the field."""
    f = Frame(800, 800, 777)
    rng = f.rng
    cell = 2 / CELLS
    kinds = ['spiral', 'spiral', 'elliptical', 'edge', 'spiral', 'irregular', 'elliptical', 'barred', 'spiral', 'lenticular',
             'spiral', 'edge', 'elliptical', 'spiral', 'irregular', 'barred', 'spiral', 'elliptical', 'edge', 'spiral',
             'lenticular', 'spiral', 'elliptical', 'irregular', 'spiral']
    for index, kind in enumerate(kinds):
        cx = -1 + cell * (index % CELLS + .5)
        cy = -1 + cell * (index // CELLS + .5)
        half = int(f.w / CELLS / 2)
        ix, iy = int(round((cx + 1) / 2 * f.w)), int(round((cy + 1) / 2 * f.h))
        sl = (slice(iy - half, iy + half), slice(ix - half, ix + half))
        X, Y = f.x[sl] - cx, f.y[sl] - cy
        R = cell * .4
        angle = rng.uniform(0, math.pi)
        U = (X * math.cos(angle) + Y * math.sin(angle)) / R
        V = (-X * math.sin(angle) + Y * math.cos(angle)) / R
        fine = noise(U.shape, 2.2 * SS, 900 + index)
        warm = GOLD * rng.uniform(.9, 1.05)
        # Each cell keeps its light inside itself.
        window = (smoothstep(1.25, 1.0, np.abs(X) / R) * smoothstep(1.25, 1.0, np.abs(Y) / R))[..., None]
        before = f.light[sl].copy()
        if kind in ('spiral', 'barred'):
            r = np.hypot(U, V)
            th = np.arctan2(V, U)
            arms = int(rng.choice([2, 2, 3]))
            k = 1 / math.tan(math.radians(rng.uniform(14, 26)))
            r0 = .3 if kind == 'barred' else .02
            phi = arms * (th - k * np.log(np.maximum(r, 1e-3) / max(r0, .05)))
            c = (np.cos(phi) + 1) / 2
            env = smoothstep(r0 * .8, r0 + .15, r) * (1 - smoothstep(.72, 1.0, r))
            sharp, clumpy, scale = rng.uniform(1.8, 4.5), rng.uniform(.8, 1.8), rng.uniform(.28, .46)
            armlight = env * (c ** sharp * (.35 + clumpy * fine) + c ** 1.2 * rng.uniform(.25, .55))
            cool = BLUE if rng.uniform() < .65 else PALE
            reach = rng.uniform(.4, .85)
            warmth = (1 - smoothstep(.12, reach, r))[..., None]
            light = (np.exp(-r / scale) * (rng.uniform(.18, .4) + 2.1 * armlight))[..., None] * (cool * (1 - warmth) + warm * warmth)
            lane = ((np.cos(phi + .55) + 1) / 2) ** 10 * env
            light *= 1 - np.clip(.7 * lane, 0, .8)[..., None] * DUST
            light += (np.exp(-(r / rng.uniform(.05, .12)) ** .75 * 1.9) * 2.2)[..., None] * WARM
            if kind == 'barred':
                light += (np.exp(-(U / .3) ** 4 - (V / .06) ** 2) * .9)[..., None] * WARM
            f.light[sl] += light
            n = 60
            rr = rng.uniform(.25, .9, n)
            m = rng.integers(0, arms, n)
            tt = k * np.log(rr / max(r0, .05)) + 2 * math.pi * m / arms + rng.normal(0, .08, n) + angle
            f.stars(cx + np.cos(tt) * rr * R, cy + np.sin(tt) * rr * R, rng.uniform(.4, 1.2, n),
                    np.where(rng.uniform(0, 1, n)[:, None] < .4, PINK, PALE), size=.0018)
        elif kind in ('elliptical', 'lenticular'):
            q = rng.uniform(.55, 1) if kind == 'elliptical' else rng.uniform(.3, .5)
            d = np.hypot(U, V / q)
            color = np.array([1, rng.uniform(.62, .8), rng.uniform(.36, .55)])
            light = np.exp(-(d / .06) ** .55 * 2.4) * 2.4 + np.exp(-(d / .32) ** 1.1) * .5
            if kind == 'lenticular':
                light += np.exp(-np.abs(V) / .04 - (np.abs(U) / .8) ** 2) * .5
            f.light[sl] += light[..., None] * color
            gr = rng.pareto(2.2, 14) * .15 + .25
            gt = rng.uniform(0, 2 * math.pi, 14)
            f.stars(cx + np.cos(gt) * gr * R, cy + np.sin(gt) * gr * R * q, rng.uniform(.15, .35, 14), GOLD, size=.0013)
        elif kind == 'edge':
            disc = np.exp(-np.abs(V) / .05 - np.abs(U) / .38) * (np.abs(U) < 1)
            bulge = np.exp(-np.hypot(U, V / .7) / .09) * 1.4
            lane = np.exp(-((V - .012) / .018) ** 2) * (np.abs(U) < .85)
            light = disc[..., None] * PALE * 1.4 + bulge[..., None] * WARM
            f.light[sl] += light * (1 - np.clip(.8 * lane, 0, .85)[..., None] * DUST)
        else:
            body = np.exp(-(np.hypot(U, V / .7) / .5) ** 2)
            clumps = np.clip(fine - .5, 0, None) * 3 * body
            f.light[sl] += (clumps * 1.6 + body * .2)[..., None] * BLUE
            n = 40
            f.stars(cx + rng.normal(0, R * .3, n), cy + rng.normal(0, R * .25, n), rng.uniform(.4, 1.2, n), PALE, size=.0018)
        f.light[sl] = before + (f.light[sl] - before) * window
    return f.save('atlas', exposure=1.8, bloom=.1, halo=0, round=False)


if __name__ == '__main__':
    os.makedirs(OUT, exist_ok=True)
    # Each plate is drawn large and kept at the size it is seen at, brought down with its light premultiplied.
    SIZES = {'barred': (768, 768), 'sunflower': (512, 512), 'hoag': (384, 384), 'sombrero': (768, 384)}
    only = set(sys.argv[3].split(',')) if len(sys.argv) > 3 else None
    for draw in (whirlpool, barred, sunflower, sombrero, hoag, atlas):
        if only and draw.__name__ not in only:
            continue
        name = draw()
        if name in SIZES and SCALE == 1:
            path = os.path.join(OUT, name + '.webp')
            image = Image.open(path).convert('RGBA').convert('RGBa').resize(SIZES[name], Image.LANCZOS).convert('RGBA')
            image.save(path, 'WEBP', quality=88, alpha_quality=90, method=6)
        print(name, flush=True)
