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


def load(p: Path) -> F:
    return np.asarray(Image.open(p).convert("RGB"), dtype=np.float32) / 255


def save(p: Path, a: F) -> None:
    Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8)).save(p)


def sphere_geometry(h: int, w: int) -> tuple[F, NDArray[np.bool_]]:
    cy, cx = SPHERE_C
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
    dx = (xx - cx) / SPHERE_R
    dy = (cy - yy) / SPHERE_R
    inside = dx * dx + dy * dy < 1
    dz = np.sqrt(np.clip(1 - dx * dx - dy * dy, 0, 1))
    return np.stack([dx, dy, dz], -1).astype(np.float32), inside


def cmd_sphere(src: Path, dst: Path) -> None:
    img = load(src)
    n, inside = sphere_geometry(*img.shape[:2])
    shade = 0.6 * (AMBIENT + (1 - AMBIENT) * np.clip(n @ LIGHT, 0, 1))
    out = img.copy()
    out[inside] = shade[inside, None]
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
    l = coef[:3] / np.linalg.norm(coef[:3])
    resid = float(np.sqrt(np.mean((n[lit] @ coef[:3] + coef[3] - lum[lit]) ** 2)))
    return l.astype(np.float32), float(coef[3]), resid


def angle(a: F, b: F) -> float:
    return float(np.degrees(np.arccos(np.clip(a @ b / np.linalg.norm(a) / np.linalg.norm(b), -1, 1))))


def cmd_lights(master: Path, edits: list[Path]) -> None:
    m = load(master)
    for p in [master, *edits]:
        img = load(p)
        l, amb, resid = solve_light(img)
        _, inside = sphere_geometry(*img.shape[:2])
        drift = float(np.abs(img[~inside] - m[~inside]).mean())
        print(f"{p.name} light=({l[0]:+.2f},{l[1]:+.2f},{l[2]:+.2f}) ambient={amb:.2f} fit_rms={resid:.3f} vs_requested_top_left={angle(l, LIGHT):.0f}deg drift_outside_sphere={drift:.3f}")


def cmd_ps(out_prefix: Path, images: list[Path]) -> None:
    imgs = [load(p) for p in images]
    lights: list[F] = []
    ambs: list[float] = []
    for im in imgs:
        l, amb, _ = solve_light(im)
        lights.append(l)
        ambs.append(amb)
    L = np.stack(lights)
    chroma = np.mean([im / np.maximum(im.mean(-1, keepdims=True), 1e-3) for im in imgs], 0)
    I = np.stack([im.mean(-1) - a * 0 for im, a in zip(imgs, ambs)], -1)
    G = np.linalg.lstsq(L, I.reshape(-1, len(imgs)).T, rcond=None)[0].T.reshape(*I.shape[:2], 3)
    rho = np.linalg.norm(G, axis=-1)
    n = G / np.maximum(rho[..., None], 1e-6)
    n[..., 2] = np.abs(n[..., 2])
    n /= np.linalg.norm(n, axis=-1, keepdims=True)
    albedo = np.clip(rho[..., None] * chroma, 0, 1)
    np.save(out_prefix.with_suffix(".npy"), n.astype(np.float32))
    save(out_prefix.with_name(out_prefix.name + "-normal.png"), (n + 1) / 2)
    save(out_prefix.with_name(out_prefix.name + "-albedo.png"), albedo)
    print(f"{out_prefix.name}: lights used", np.round(L, 2).tolist(), "normal_z_mean", float(n[..., 2].mean()))


def cmd_albedo(src: Path, normals: Path, out_prefix: Path) -> None:
    img = load(src)
    n = np.load(normals).astype(np.float32)
    nl = np.clip(n @ LIGHT, 0, 1)
    shade = AMBIENT + (1 - AMBIENT) * nl
    corr = float(np.corrcoef(nl.ravel(), img.mean(-1).ravel())[0, 1])
    print(f"{src.name}: corr(n.l, luminance)={corr:+.3f} normal std={n.reshape(-1,3).std(0).round(3).tolist()}")
    albedo = np.clip(img / shade[..., None], 0, 1)
    np.save(out_prefix.with_suffix(".npy"), n)
    save(out_prefix.with_name(out_prefix.name + "-normal.png"), (n + 1) / 2)
    save(out_prefix.with_name(out_prefix.name + "-albedo.png"), albedo)


def hex_mask(h: int, w: int) -> NDArray[np.bool_]:
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
    x = (xx - w / 2) / (w / 2)
    y = (yy - h / 2) / (h / 2)
    return (np.abs(x) <= np.sqrt(3) / 2 * 0.98) & (np.abs(y) + np.abs(x) / np.sqrt(3) <= 0.98)


def rotate(a: F, deg: float) -> F:
    chans = [Image.fromarray(np.ascontiguousarray(a[..., i])).rotate(deg, Image.BICUBIC) for i in range(a.shape[-1])]
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
            nz = nr[..., 2]
            norm = np.maximum(np.sqrt(nx * nx + ny * ny + nz * nz), 1e-6)
            nl = np.clip((nx * LIGHT[0] + ny * LIGHT[1] + nz * LIGHT[2]) / norm, 0, 1)
            a = a * (AMBIENT + (1 - AMBIENT) * nl)[..., None]
        a[~mask] = 0.42
        tiles.append(np.asarray(Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8)).resize((size, size), Image.LANCZOS), dtype=np.float32) / 255)
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
