import json
import math
import os
import glob
from typing import List, Optional, Tuple

import matplotlib.pyplot as plt
from matplotlib.patches import RegularPolygon, FancyArrowPatch, Arc


def normalize_map(m):
    rooms = m["rooms"]
    N = len(rooms)
    conns = m["connections"]
    start = int(m.get("startingRoom", 0))
    return N, rooms, conns, start


def hex_door_anchor(cx, cy, door_idx, r_room=1.2):
    # door 0 at +Y (90deg), clockwise numbers
    angle = math.radians(90 - door_idx * 60.0)
    x = cx + r_room * math.cos(angle)
    y = cy + r_room * math.sin(angle)
    return x, y, angle


def draw_hex(ax, cx, cy, label_text, room_idx, start=False, r_hex=1.0):
    patch = RegularPolygon(
        (cx, cy),
        numVertices=6,
        radius=r_hex,
        orientation=math.radians(30),
        fill=False,
        lw=3 if start else 1.5,
    )
    ax.add_patch(patch)
    ax.text(
        cx, cy, f"{room_idx}\n[{label_text}]", ha="center", va="center", fontsize=10
    )


def pick_rad(qa, da, qb, db, a, b):
    # sign from a simple hash to reduce overlap
    sign = 1 if ((qa * 13 + da * 7 + qb * 11 + db * 5) % 2) else -1
    dx, dy = b[0] - a[0], b[1] - a[1]
    dist = math.hypot(dx, dy)
    # magnitude decreases with distance, clamped
    mag = 0.32 * (1.5 / (1.0 + dist / 6.0))
    mag = max(0.12, min(0.45, mag))
    return sign * mag


def draw_connection_curved(ax, a, b, qa, da, qb, db):
    # Use FancyArrowPatch with arc3 connectionstyle for a smooth curve
    rad = pick_rad(qa, da, qb, db, a, b)
    patch = FancyArrowPatch(
        a,
        b,
        arrowstyle="-",
        mutation_scale=1,
        connectionstyle=f"arc3,rad={rad}",
        lw=1.6,
    )
    ax.add_patch(patch)


def draw_self_loop(ax, anchor, angle):
    # Draw a loop that sits just outside the hex near the door angle
    # offset the loop center outward along door normal
    ux, uy = math.cos(angle), math.sin(angle)
    cx = anchor[0] + ux * 0.35
    cy = anchor[1] + uy * 0.35
    # choose arc extent
    arc = Arc(
        (cx, cy), 0.9, 0.9, angle=math.degrees(angle), theta1=210, theta2=510, lw=1.6
    )
    ax.add_patch(arc)


def layout_positions(N, radius=4.0):
    if N == 1:
        return {0: (0.0, 0.0)}
    pos = {}
    for i in range(N):
        theta = 2.0 * math.pi * i / N
        pos[i] = (radius * math.cos(theta), radius * math.sin(theta))
    return pos


def draw_map_on_axes(ax, map_json):
    """Draw a map JSON on a provided Axes, clearing it first.

    Returns a tuple of (fig, ax) for convenience.
    """
    ax.clear()
    N, rooms, conns, start = normalize_map(map_json)
    pos = layout_positions(N, radius=max(4.0, 2.0 + 0.6 * N))

    r_hex = 1.0
    door_anchor = {}  # (room, door) -> (x,y,angle)
    for q in range(N):
        cx, cy = pos[q]
        draw_hex(ax, cx, cy, rooms[q], q, start=(q == start), r_hex=r_hex)
        for d in range(6):
            ax_x, ax_y, ang = hex_door_anchor(cx, cy, d, r_room=r_hex * 1.2)
            door_anchor[(q, d)] = (ax_x, ax_y, ang)
            ax.text(ax_x, ax_y, str(d), ha="center", va="center", fontsize=8)

    # Curved connections
    drawn = set()
    for item in conns:
        qa = int(item["from"]["room"])
        da = int(item["from"]["door"])
        qb = int(item["to"]["room"])
        db = int(item["to"]["door"])
        key = tuple(sorted([(qa, da), (qb, db)]))
        if key in drawn:
            continue
        drawn.add(key)
        a = door_anchor.get((qa, da))
        b = door_anchor.get((qb, db))
        if a is None or b is None:
            continue
        (x1, y1, aang) = a
        (x2, y2, bang) = b
        if abs(x1 - x2) < 1e-8 and abs(y1 - y2) < 1e-8:
            draw_self_loop(ax, (x1, y1), aang)
        else:
            draw_connection_curved(ax, (x1, y1), (x2, y2), qa, da, qb, db)

    ax.set_aspect("equal", adjustable="datalim")
    ax.set_axis_off()
    ax.relim()
    ax.autoscale_view()
    return ax.figure, ax


def visualize_map(map_json, output=None, figsize=(8, 8)):
    fig, ax = plt.subplots(figsize=figsize)
    draw_map_on_axes(ax, map_json)
    if output:
        plt.savefig(output, bbox_inches="tight", dpi=220)
    return fig, ax


def list_sequence_files(pattern: str) -> List[str]:
    """Return a lexicographically sorted list of files for a glob pattern."""
    files = glob.glob(pattern)
    files.sort()
    return files


def load_json(path: str):
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def interactive_browse(files: List[str], figsize: Tuple[float, float] = (8, 8)):
    """Open a window and allow stepping through frames with arrow keys.

    Keys: left/right (or 'p'/'n'), space=next, home/end, 'q' to close.
    """
    if not files:
        raise ValueError("No input files to browse.")
    fig, ax = plt.subplots(figsize=figsize)
    state = {"idx": 0}

    def render():
        i = state["idx"]
        m = load_json(files[i])
        draw_map_on_axes(ax, m)
        fig.suptitle(f"{i+1}/{len(files)}: {os.path.basename(files[i])}")
        fig.canvas.draw_idle()

    def on_key(event):
        key = (event.key or "").lower()
        if key in ("right", " ", "n"):
            state["idx"] = (state["idx"] + 1) % len(files)
            render()
        elif key in ("left", "p"):
            state["idx"] = (state["idx"] - 1) % len(files)
            render()
        elif key == "home":
            state["idx"] = 0
            render()
        elif key == "end":
            state["idx"] = len(files) - 1
            render()
        elif key == "q":
            plt.close(fig)

    fig.canvas.mpl_connect("key_press_event", on_key)
    render()
    plt.show()


def export_animation(
    files: List[str],
    output_path: str,
    figsize: Tuple[float, float] = (8, 8),
    fps: float = 2.0,
    save_frames_dir: Optional[str] = None,
):
    """Export an animation from a sequence of map jsons.

    Supports .gif natively. Attempts .apng if Pillow supports it; otherwise
    falls back to saving per-frame PNGs when save_frames_dir is provided.
    """
    if not files:
        raise ValueError("No input files to animate.")

    # Prepare figure reused for each frame
    fig, ax = plt.subplots(figsize=figsize)
    # Ensure an Agg canvas for consistent pixel buffer access
    try:
        from matplotlib.backends.backend_agg import FigureCanvasAgg  # type: ignore

        FigureCanvasAgg(fig)  # attaches an Agg canvas to the figure
        has_agg = True
    except Exception:
        has_agg = False

    # Optionally save frames individually
    if save_frames_dir:
        os.makedirs(save_frames_dir, exist_ok=True)

    # Try PIL for animated outputs
    pil_ok = False
    try:
        from PIL import Image
        import numpy as np

        pil_ok = True
    except Exception:
        Image = None  # type: ignore
        np = None  # type: ignore

    frames = []
    for i, path in enumerate(files):
        m = load_json(path)
        draw_map_on_axes(ax, m)
        fig.suptitle(f"{i+1}/{len(files)}: {os.path.basename(path)}")
        # Save individual frame if requested
        if save_frames_dir:
            out_png = os.path.join(save_frames_dir, f"frame-{i:06d}.png")
            fig.savefig(out_png, bbox_inches="tight", dpi=220)

        if pil_ok:
            # Render canvas and grab a pixel buffer
            fig.canvas.draw()
            w, h = fig.canvas.get_width_height()
            if has_agg and hasattr(fig.canvas, "buffer_rgba"):
                # Agg backend: reliable RGBA buffer
                buf = fig.canvas.buffer_rgba()
                tmp = np.frombuffer(buf, dtype=np.uint8).reshape((h, w, 4))
                arr = tmp[:, :, :3].copy()  # RGBA -> RGB
            elif hasattr(fig.canvas, "tostring_rgb"):
                buf = fig.canvas.tostring_rgb()
                arr = np.frombuffer(buf, dtype=np.uint8).reshape((h, w, 3))
            else:
                # Fallback for backends providing ARGB only
                buf = fig.canvas.tostring_argb()
                tmp = np.frombuffer(buf, dtype=np.uint8).reshape((h, w, 4))
                # ARGB -> RGB by dropping alpha and reordering
                arr = tmp[:, :, 1:4].copy()
            frames.append(Image.fromarray(arr, mode="RGB"))

    # Attempt to save animation if PIL available
    ext = os.path.splitext(output_path)[1].lower()
    if pil_ok and frames:
        duration_ms = int(1000.0 / max(1e-9, fps))
        if ext == ".gif":
            frames[0].save(
                output_path,
                save_all=True,
                append_images=frames[1:],
                duration=duration_ms,
                loop=0,
                optimize=False,
            )
            return
        elif ext in (".apng", ".png"):
            try:
                frames[0].save(
                    output_path,
                    save_all=True,
                    append_images=frames[1:],
                    duration=duration_ms,
                    loop=0,
                    optimize=False,
                    format="PNG",
                )
                return
            except Exception:
                # Fall through to frame dump if not supported
                pass

    # If we get here, we couldn't write the requested animation directly
    if not save_frames_dir:
        raise RuntimeError(
            "Animated export requires Pillow for .gif/.apng. "
            "Rerun with --save-frames to get per-frame PNGs."
        )

    # Otherwise, frames were saved individually already.
    print(
        f"Saved {len(files)} frames under '{save_frames_dir}'. Combine externally if needed."
    )


def main():
    import argparse

    ap = argparse.ArgumentParser(
        description="Curved visualization for ICFP 2025 Ædificium map"
    )
    ap.add_argument("--input", help="Single map.json (backward compatible)")
    ap.add_argument(
        "--glob",
        help="Glob for sequence, e.g. 'example/map-probatio-solver-*.json'",
    )
    ap.add_argument(
        "--output", default=None, help="output image path (e.g., map_curvy.png)"
    )
    ap.add_argument(
        "--animate-output",
        default=None,
        help="Animation file path (.gif or .apng). Implies --glob",
    )
    ap.add_argument(
        "--save-frames",
        default=None,
        help="Directory to save per-frame PNGs when animating",
    )
    ap.add_argument("--fps", type=float, default=2.0, help="Animation FPS")
    ap.add_argument("--figsize", default="8,8", help="figure size W,H")
    args = ap.parse_args()

    W, H = [float(x) for x in args.figsize.split(",")]

    if args.animate_output or args.glob:
        if not args.glob:
            raise SystemExit("--animate-output requires --glob to specify frames")
        files = list_sequence_files(args.glob)
        if not files:
            raise SystemExit(f"No files matched glob: {args.glob}")
        if args.animate_output:
            export_animation(
                files,
                args.animate_output,
                figsize=(W, H),
                fps=args.fps,
                save_frames_dir=args.save_frames,
            )
        else:
            # Interactive stepping through frames
            interactive_browse(files, figsize=(W, H))
        return

    # Single-file mode (default/backward-compatible)
    if not args.input:
        raise SystemExit("Either --input or --glob is required")

    with open(args.input, "r", encoding="utf-8") as f:
        m = json.load(f)

    visualize_map(m, output=args.output, figsize=(W, H))
    if not args.output:
        plt.show()


if __name__ == "__main__":
    main()
