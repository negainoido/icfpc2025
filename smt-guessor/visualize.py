import json, math
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


def visualize_map(map_json, output=None, figsize=(8, 8)):
    N, rooms, conns, start = normalize_map(map_json)
    pos = layout_positions(N, radius=max(4.0, 2.0 + 0.6 * N))
    fig, ax = plt.subplots(figsize=figsize)

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

    if output:
        plt.savefig(output, bbox_inches="tight", dpi=220)
    return fig, ax


def main():
    import argparse

    ap = argparse.ArgumentParser(
        description="Curved visualization for ICFP 2025 Ædificium map"
    )
    ap.add_argument("--input", required=True, help="map.json")
    ap.add_argument(
        "--output", default=None, help="output image path (e.g., map_curvy.png)"
    )
    ap.add_argument("--figsize", default="8,8", help="figure size W,H")
    args = ap.parse_args()

    with open(args.input, "r", encoding="utf-8") as f:
        m = json.load(f)

    W, H = [float(x) for x in args.figsize.split(",")]
    visualize_map(m, output=args.output, figsize=(W, H))
    if not args.output:
        plt.show()


if __name__ == "__main__":
    main()
