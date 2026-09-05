#!/usr/bin/env python3
import sys
from pathlib import Path
import numpy as np
from numpy.typing import NDArray
from PIL import Image

F = NDArray[np.float32]
LIGHT: F = np.array([-0.5, 0.5, 0.7], dtype=np.float32)
LIGHT /= np.linalg.norm(LIGHT)
AMBIENT = 0.25
SPHERE_R = 96
SPHERE_C = (1024 - SPHERE_R - 32, 1024 - SPHERE_R - 32)
REQUESTED: dict[str, tuple[float, float, float]] = {
    "master": (float(LIGHT[0]), float(LIGHT[1]), float(LIGHT[2])),
    "right": (1, 0, 1),
    "top-right": (1, 1, 1),
    "left": (-1, 0, 1),
    "bottom": (0, -1, 1),
}


def load(p: Path) -> F:
    return (np.asarray(Image.open(p).convert("RGB"), dtype=np.float32) / 255).astype(np.float32)


def save(p: Path, a: F) -> None:
    Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8)).save(p)


def unit(v: F) -> F:
    return (v / np.linalg.norm(v)).astype(np.float32)


def angle(a: F, b: F) -> float:
    return float(np.degrees(np.arccos(np.clip(unit(a) @ unit(b), -1, 1))))


def shade(n: F) -> F:
    nl = np.clip(n @ LIGHT, 0, 1)
    return np.asarray((AMBIENT + (1 - AMBIENT) * nl) / (AMBIENT + (1 - AMBIENT) * LIGHT[2]), dtype=np.float32)


def flatten(n: F) -> F:
    m = unit(n.reshape(-1, 3).mean(0))
    z = np.array([0, 0, 1], dtype=np.float32)
    axis = np.cross(m, z).astype(np.float32)
    s = float(np.linalg.norm(axis))
    if s < 1e-6:
        return n
    axis /= s
    c = float(m @ z)
    k = np.array([[0, -axis[2], axis[1]], [axis[2], 0, -axis[0]], [-axis[1], axis[0], 0]], dtype=np.float32)
    r = np.eye(3, dtype=np.float32) + s * k + (1 - c) * (k @ k)
    out = n @ r.T
    return np.asarray(out / np.linalg.norm(out, axis=-1, keepdims=True), dtype=np.float32)


def requested_light(p: Path) -> F:
    key = next(k for k in REQUESTED if p.stem.endswith(k) or p.stem.endswith(k + "-1024"))
    return unit(np.array(REQUESTED[key], dtype=np.float32))


def sphere_geometry(h: int, w: int) -> tuple[F, NDArray[np.bool_]]:
    cy, cx = SPHERE_C
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
    dx = (xx - cx) / SPHERE_R
    dy = (cy - yy) / SPHERE_R
    inside = dx * dx + dy * dy < 1
    dz = np.sqrt(np.clip(1 - dx * dx - dy * dy, 0, 1))
    return np.stack([dx, dy, dz], -1).astype(np.float32), inside.astype(np.bool_)


def cmd_sphere(src: Path, dst: Path) -> None:
    img = load(src)
    n, inside = sphere_geometry(*img.shape[:2])
    lum = 0.6 * (AMBIENT + (1 - AMBIENT) * np.clip(n @ LIGHT, 0, 1))
    out = img.copy()
    out[inside] = lum[inside, None]
    save(dst, out)


def solve_light(img: F) -> tuple[F, float, float]:
    n, inside = sphere_geometry(*img.shape[:2])
    lum = img.mean(-1)
    lit = inside & (n[..., 2] > 0.15)
    for _ in range(3):
        A = np.concatenate([n[lit], np.ones((lit.sum(), 1), np.float32)], 1)
        coef, *_ = np.linalg.lstsq(A, lum[lit], rcond=None)
        pred = n[inside] @ coef[:3] + coef[3]
        lit = inside.copy()
        lit[inside] = pred > coef[3] + 0.02 * np.abs(coef[:3]).sum()
    resid = float(np.sqrt(np.mean((n[lit] @ coef[:3] + coef[3] - lum[lit]) ** 2)))
    return unit(coef[:3]), float(coef[3]), resid


def cmd_lights(master: Path, edits: list[Path]) -> None:
    m = load(master)
    for p in [master, *edits]:
        img = load(p)
        l, amb, resid = solve_light(img)
        _, inside = sphere_geometry(*img.shape[:2])
        drift = float(np.abs(img[~inside] - m[~inside]).mean())
        print(f"{p.name} light=({l[0]:+.2f},{l[1]:+.2f},{l[2]:+.2f}) ambient={amb:.2f} fit_rms={resid:.3f} off_requested={angle(l, requested_light(p)):.0f}deg drift_outside_sphere={drift:.3f}")


def write_maps(out_prefix: Path, n: F, albedo: F) -> None:
    np.save(out_prefix.with_suffix(".npy"), n.astype(np.float32))
    save(out_prefix.with_name(out_prefix.name + "-normal.png"), ((n + 1) / 2).astype(np.float32))
    save(out_prefix.with_name(out_prefix.name + "-albedo.png"), albedo)


def cmd_ps(out_prefix: Path, images: list[Path]) -> None:
    imgs: list[F] = []
    lights: list[F] = []
    for p in images:
        im = load(p)
        l, _, _ = solve_light(im)
        if l[2] < 0.05:
            print(f"{p.name}: light z={l[2]:+.2f} is below the surface plane, clamped to a raking light")
            l = unit(np.array([l[0], l[1], 0.05], dtype=np.float32))
        imgs.append(im)
        lights.append(l)
    L = np.stack(lights)
    chroma = np.mean([im / np.maximum(im.mean(-1, keepdims=True), 1e-3) for im in imgs], 0)
    I = np.stack([im.mean(-1) for im in imgs], -1)
    G = np.linalg.lstsq(L, I.reshape(-1, len(imgs)).T, rcond=None)[0].T.reshape(*I.shape[:2], 3)
    rho = np.linalg.norm(G, axis=-1)
    n = G / np.maximum(rho[..., None], 1e-6)
    n[..., 2] = np.abs(n[..., 2])
    n = flatten(n / np.linalg.norm(n, axis=-1, keepdims=True))
    rho *= float(np.median(imgs[0].mean(-1)) / np.median(rho * shade(n)))
    write_maps(out_prefix, n, np.clip(rho[..., None] * chroma, 0, 1))
    print(f"{out_prefix.name}: {len(imgs)} images, condition={np.linalg.cond(L):.1f}, normal std={n.reshape(-1, 3).std(0).round(3).tolist()}")


def cmd_albedo(src: Path, normals: Path, out_prefix: Path) -> None:
    img = load(src)
    n = flatten(np.load(normals).astype(np.float32))
    s = shade(n)
    corr = float(np.corrcoef(s.ravel(), img.mean(-1).ravel())[0, 1])
    print(f"{src.name}: corr(shade, luminance)={corr:+.3f} normal std={n.reshape(-1, 3).std(0).round(3).tolist()}")
    write_maps(out_prefix, n, np.clip(img / s[..., None], 0, 1))


def hex_mask(h: int, w: int) -> NDArray[np.bool_]:
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
    x = (xx - w / 2) / (w / 2)
    y = (yy - h / 2) / (h / 2)
    return np.asarray((np.abs(x) <= np.sqrt(3) / 2 * 0.98) & (np.abs(y) + np.abs(x) / np.sqrt(3) <= 0.98), dtype=np.bool_)


def rotate(a: F, deg: float) -> F:
    chans = [Image.fromarray(np.ascontiguousarray(a[..., i])).rotate(deg, Image.Resampling.BICUBIC) for i in range(a.shape[-1])]
    return np.stack([np.asarray(c, dtype=np.float32) for c in chans], -1)


def cmd_render(prefix: Path, albedo: Path, normals: Path | None, size: int) -> None:
    alb = load(albedo)
    n = np.load(normals).astype(np.float32) if normals else None
    mask = hex_mask(*alb.shape[:2])
    tiles: list[F] = []
    for k in range(6):
        deg = 60 * k
        th = np.radians(deg)
        c, s = np.cos(th), np.sin(th)
        a = rotate(alb, deg)
        if n is not None:
            nr = rotate(n, deg)
            nx = c * nr[..., 0] - s * nr[..., 1]
            ny = s * nr[..., 0] + c * nr[..., 1]
            rotated = np.stack([nx, ny, nr[..., 2]], -1)
            rotated /= np.maximum(np.linalg.norm(rotated, axis=-1, keepdims=True), 1e-6)
            a = a * shade(rotated)[..., None]
        a[~mask] = 0.42
        small = Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8)).resize((size, size), Image.Resampling.LANCZOS)
        tiles.append((np.asarray(small, dtype=np.float32) / 255).astype(np.float32))
    save(prefix.with_suffix(".png"), np.concatenate(tiles, 1))


def main() -> None:
    cmd, *args = sys.argv[1:]
    if cmd == "sphere":
        cmd_sphere(Path(args[0]), Path(args[1]))
    elif cmd == "lights":
        cmd_lights(Path(args[0]), [Path(a) for a in args[1:]])
    elif cmd == "ps":
        cmd_ps(Path(args[0]), [Path(a) for a in args[1:]])
    elif cmd == "albedo":
        cmd_albedo(Path(args[0]), Path(args[1]), Path(args[2]))
    elif cmd == "render":
        cmd_render(Path(args[0]), Path(args[1]), Path(args[2]) if args[2] != "-" else None, int(args[3]))
    else:
        raise SystemExit(f"unknown command {cmd}")


if __name__ == "__main__":
    main()
