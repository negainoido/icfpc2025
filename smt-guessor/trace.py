import argparse
import json
from dataclasses import dataclass
from typing import Dict, List, Tuple

import matplotlib.pyplot as plt
from matplotlib.patches import RegularPolygon, Arc, Rectangle

# Reuse helpers from visualize.py to keep a consistent look
from visualize import (
    normalize_map,  # type: ignore
    layout_positions,  # type: ignore
    hex_door_anchor,  # type: ignore
    draw_map_on_axes,  # type: ignore
    pick_rad,  # type: ignore
)
from matplotlib.patches import FancyArrowPatch


@dataclass
class Step:
    index: int  # observation index in results (0-based)
    from_room: int
    door: int  # -1 indicates observation at current room (no move)
    to_room: int
    observed: int
    expected: int
    mismatch: bool


def build_conn_map(conns: List[dict]) -> Dict[Tuple[int, int], Tuple[int, int]]:
    """Create a bidirectional mapping (room, door) -> (room, door)."""
    m: Dict[Tuple[int, int], Tuple[int, int]] = {}
    for c in conns:
        a = (int(c["from"]["room"]), int(c["from"]["door"]))
        b = (int(c["to"]["room"]), int(c["to"]["door"]))
        m[a] = b
        m[b] = a
    return m


def simulate_steps(map_json: dict, trace_json: dict, plan_index: int = 0) -> List[Step]:
    """Follow the plan over the map, pairing each move with an observation.

    Assumes a plan is a string of digits (0..5) indicating doors to take.
    Observations are taken from trace_json["results"][plan_index], and compared
    to the label of the room just entered.
    """
    N, rooms, conns, start = normalize_map(map_json)
    conn = build_conn_map(conns)

    # Select plan and results
    plans: List[str] = trace_json.get("plans", [])
    if not plans:
        raise ValueError("Trace JSON has no 'plans'.")
    if not (0 <= plan_index < len(plans)):
        raise ValueError(f"plan_index {plan_index} out of range (0..{len(plans)-1})")
    plan = plans[plan_index]
    res_all: List[List[int]] = trace_json.get("results", [])
    if not res_all:
        raise ValueError("Trace JSON has no 'results'.")
    results = res_all[plan_index]

    # Observations: results[0] compares the startingRoom's label
    cur = int(trace_json.get("startingRoom", start))
    steps: List[Step] = []

    if len(results) >= 1:
        obs0 = int(results[0])
        exp0 = int(rooms[cur])
        steps.append(
            Step(index=0, from_room=cur, door=-1, to_room=cur, observed=obs0, expected=exp0, mismatch=(obs0 != exp0))
        )

    # Subsequent observations correspond to moves in the plan
    # results[i+1] compares the label of the room reached after plan[i]
    max_moves = min(len(plan), max(0, len(results) - 1))
    for i in range(max_moves):
        dch = plan[i]
        if dch < "0" or dch > "9":
            raise ValueError(f"Invalid door digit '{dch}' at plan index {i}")
        door = int(dch)
        if door < 0 or door > 5:
            # Only doors 0..5 are valid
            continue
        key = (cur, door)
        if key not in conn:
            # No connection: remain in place
            nxt = cur
        else:
            nxt, _ = conn[key]
        exp = int(rooms[nxt])
        obs = int(results[i + 1])
        steps.append(
            Step(index=i + 1, from_room=cur, door=door, to_room=nxt, observed=obs, expected=exp, mismatch=(obs != exp))
        )
        cur = nxt

    return steps


def _build_rad_cache(map_json: dict, pos: Dict[int, Tuple[float, float]], r_hex: float = 1.0):
    """Precompute the exact curvature (rad) used in the base map for each edge.

    Returns mapping: key = tuple(sorted([(qa,da),(qb,db)])) -> {
        'rad': float,
        'a': (qa,da),
        'b': (qb,db)
    } where (qa,da)->(qb,db) is the orientation actually drawn in the base map.
    """
    conns = map_json["connections"]
    door_anchor: Dict[Tuple[int, int], Tuple[float, float, float]] = {}
    # prime anchors
    for q, (cx, cy) in pos.items():
        for d in range(6):
            door_anchor[(q, d)] = hex_door_anchor(cx, cy, d, r_room=r_hex * 1.2)

    rad_cache: Dict[Tuple[Tuple[int, int], Tuple[int, int]], Dict] = {}
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
        a = door_anchor[(qa, da)]
        b = door_anchor[(qb, db)]
        rad = pick_rad(qa, da, qb, db, (pos[qa][0], pos[qa][1]), (pos[qb][0], pos[qb][1]))
        rad_cache[key] = {"rad": rad, "a": (qa, da), "b": (qb, db)}
    return rad_cache


def draw_overlay_for_step(ax, map_json: dict, step: Step, pos: Dict[int, Tuple[float, float]], r_hex: float = 1.0, rad_cache: Dict = None):
    """Overlay current step path and target room highlight."""
    color = "red" if step.mismatch else "tab:green"

    # Highlight the entered room with a semi-transparent fill ring
    cx, cy = pos[step.to_room]
    ring = RegularPolygon(
        (cx, cy), numVertices=6, radius=r_hex * 1.02, orientation=0.5235987755982988, fill=False, lw=4.0, edgecolor=color
    )
    ax.add_patch(ring)

    # If this step is an observation at current room (no move), skip drawing the edge
    if step.door >= 0:
        # Compute door anchors for from and to
        a_x, a_y, _ = hex_door_anchor(
            pos[step.from_room][0], pos[step.from_room][1], step.door, r_room=r_hex * 1.2
        )
        # Find paired door on the other side
        conn = build_conn_map(map_json["connections"])  # type: ignore
        to_pair = conn.get((step.from_room, step.door), (step.to_room, 0))
        to_door = to_pair[1]
        b_x, b_y, _ = hex_door_anchor(
            pos[step.to_room][0], pos[step.to_room][1], to_door, r_room=r_hex * 1.2
        )

        # Determine curvature exactly as base map used
        edge_key = tuple(sorted([(step.from_room, step.door), (step.to_room, to_door)]))
        if rad_cache and edge_key in rad_cache:
            info = rad_cache[edge_key]
            # If our orientation matches the stored one, use rad; otherwise invert sign
            if info["a"] == (step.from_room, step.door) and info["b"] == (step.to_room, to_door):
                rad = info["rad"]
            else:
                rad = -info["rad"]
        else:
            # Fallback to local computation
            rad = pick_rad(
                step.from_room,
                step.door,
                step.to_room,
                to_door,
                (pos[step.from_room][0], pos[step.from_room][1]),
                (pos[step.to_room][0], pos[step.to_room][1]),
            )

        patch = FancyArrowPatch(
            (a_x, a_y),
            (b_x, b_y),
            arrowstyle="-",
            mutation_scale=1,
            connectionstyle=f"arc3,rad={rad}",
            lw=3.5,
            color=color,
            alpha=0.95,
            zorder=50,
        )
        ax.add_patch(patch)
    # Self-loop overlay ONLY when the map edge is (room, door) -> (room, door)
    # i.e., same room and same door on both ends. For same room but different
    # doors, base map uses a curved connection between two anchors, so we skip
    # the special loop here to match exactly.
    if step.door >= 0 and step.from_room == step.to_room and to_door == step.door:
        # place a loop outside the hex near the door angle, matching visualize.draw_self_loop geometry
        import math as _math
        ax_x, ax_y, ang = hex_door_anchor(
            pos[step.from_room][0], pos[step.from_room][1], step.door, r_room=r_hex * 1.2
        )
        ux, uy = _math.cos(ang), _math.sin(ang)
        cx = ax_x + ux * 0.35
        cy = ax_y + uy * 0.35
        arc = Arc(
            (cx, cy), 0.9, 0.9, angle=_math.degrees(ang), theta1=210, theta2=510, lw=3.2, color=color, zorder=50
        )
        ax.add_patch(arc)


def compute_mismatch_rooms(steps: List[Step]) -> List[int]:
    seen = set()
    for s in steps:
        if s.mismatch:
            seen.add(s.to_room)
    return sorted(seen)


def draw_all_mismatch_highlights(ax, mismatch_rooms: List[int], pos: Dict[int, Tuple[float, float]], r_hex: float = 1.0):
    for q in mismatch_rooms:
        cx, cy = pos[q]
        patch = RegularPolygon(
            (cx, cy),
            numVertices=6,
            radius=r_hex * 0.95,
            orientation=0.5235987755982988,
            fill=True,
            facecolor=(1.0, 0.0, 0.0, 0.12),
            edgecolor=None,
            lw=0,
        )
        ax.add_patch(patch)


def interactive_trace(map_json: dict, trace_json: dict, plan_index: int = 0, figsize=(8, 8)):
    """Interactive viewer: step through trace with arrow keys, showing mismatches.

    Keys: left/right, space=next, home/end, 'm' toggle mismatch heat, 'q' quit.
    """
    steps = simulate_steps(map_json, trace_json, plan_index=plan_index)
    N, rooms, conns, start = normalize_map(map_json)
    fig, ax = plt.subplots(figsize=figsize)
    # A slim progress bar axes at the top (below the title)
    progress_ax = fig.add_axes([0.08, 0.93, 0.84, 0.025])
    progress_ax.set_axis_off()

    # Layout consistent with visualize.draw_map_on_axes
    pos = layout_positions(N, radius=max(4.0, 2.0 + 0.6 * N))
    r_hex = 1.0
    mismatch_rooms = compute_mismatch_rooms(steps)
    mismatch_indices = [i for i, st in enumerate(steps) if st.mismatch]
    rad_cache = _build_rad_cache(map_json, pos, r_hex=r_hex)

    state = {"idx": 0, "show_mismatch": True}

    # Footer: small English controls help
    footer_text = (
        "Controls: Left/Right or P/N, Space=next, Home/End, [=prev mismatch, ]=next mismatch, M=toggle mismatch rooms, Q=quit"
    )
    footer = fig.text(
        0.5,
        0.015,
        footer_text,
        ha="center",
        va="bottom",
        fontsize=8,
        color="dimgray",
        bbox=dict(facecolor="white", alpha=0.6, edgecolor="none", pad=2),
        zorder=100,
    )

    def draw_progress_bar():
        progress_ax.clear()
        progress_ax.set_axis_off()
        L = len(steps)
        if L == 0:
            return
        # Normalize x from 0..L
        progress_ax.set_xlim(0, L)
        progress_ax.set_ylim(0, 1)
        # Draw base segments: red where mismatch, light gray otherwise
        for i, st in enumerate(steps):
            color = (0.85, 0.15, 0.15) if st.mismatch else (0.82, 0.82, 0.82)
            rect = Rectangle((i, 0), 1.0, 1.0, facecolor=color, edgecolor=None, lw=0)
            progress_ax.add_patch(rect)
        # Highlight current index with an outline
        ci = state["idx"]
        outline = Rectangle((ci, 0), 1.0, 1.0, facecolor='none', edgecolor='black', lw=1.5)
        progress_ax.add_patch(outline)

    def render():
        draw_map_on_axes(ax, map_json)
        if state["show_mismatch"] and mismatch_rooms:
            draw_all_mismatch_highlights(ax, mismatch_rooms, pos, r_hex=r_hex)
        if steps:
            s = steps[state["idx"]]
            draw_overlay_for_step(ax, map_json, s, pos, r_hex=r_hex, rad_cache=rad_cache)
            if s.door < 0:
                title = (
                    f"obs {s.index+1}/{len(steps)}: room {s.to_room} | expected={s.expected} observed={s.observed}"
                )
            else:
                title = (
                    f"step {s.index}/{len(steps)-1}: room {s.from_room} --d{s.door}--> room {s.to_room} | expected={s.expected} observed={s.observed}"
                )
            if s.mismatch:
                title += "  [MISMATCH]"
            fig.suptitle(title, y=0.985)
        draw_progress_bar()
        fig.canvas.draw_idle()

    def on_key(event):
        key = (event.key or "").lower()
        if key in ("right", " ", "n"):
            state["idx"] = (state["idx"] + 1) % max(1, len(steps))
            render()
        elif key in ("left", "p"):
            state["idx"] = (state["idx"] - 1) % max(1, len(steps))
            render()
        elif key == "home":
            state["idx"] = 0
            render()
        elif key == "end":
            state["idx"] = max(0, len(steps) - 1)
            render()
        elif key in ("[", "{"):
            # previous mismatch
            if mismatch_indices:
                cur = state["idx"]
                prev = None
                for idx in reversed(mismatch_indices):
                    if idx < cur:
                        prev = idx
                        break
                if prev is None:
                    prev = mismatch_indices[-1]
                state["idx"] = prev
                render()
        elif key in ("]", "}"):
            # next mismatch
            if mismatch_indices:
                cur = state["idx"]
                nxt = None
                for idx in mismatch_indices:
                    if idx > cur:
                        nxt = idx
                        break
                if nxt is None:
                    nxt = mismatch_indices[0]
                state["idx"] = nxt
                render()
        elif key == "m":
            state["show_mismatch"] = not state["show_mismatch"]
            render()
        elif key == "q":
            plt.close(fig)

    fig.canvas.mpl_connect("key_press_event", on_key)
    render()
    plt.show()


def main():
    ap = argparse.ArgumentParser(description="Interactive trace visualizer over a map")
    ap.add_argument("--trace", required=True, help="Trace JSON (e.g., example/probatio.json)")
    ap.add_argument("--map", required=True, help="Map JSON (e.g., example/map-probatio.json)")
    ap.add_argument("--plan-index", type=int, default=0, help="Select which plan/results index to visualize")
    ap.add_argument("--figsize", default="8,8", help="figure size W,H")
    args = ap.parse_args()

    W, H = [float(x) for x in args.figsize.split(",")]

    with open(args.map, "r", encoding="utf-8") as f:
        map_json = json.load(f)
    with open(args.trace, "r", encoding="utf-8") as f:
        trace_json = json.load(f)

    interactive_trace(map_json, trace_json, plan_index=args.plan_index, figsize=(W, H))


if __name__ == "__main__":
    main()
