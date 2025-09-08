#!/usr/bin/env python3
"""
Compute an Euler path for a map graph in this repo's JSON format.

Input schema (same as produced by main.py / used by visualize.py):
- rooms: list[int]
- startingRoom: int (optional)
- connections: list of {from:{room,door}, to:{room,door}}

We treat the graph as an undirected multigraph on rooms. Each connection is
one edge between the two endpoint rooms (self-loops allowed). If an Euler
trail exists (0 or 2 odd-degree vertices), we return an ordering of edge
indices that traverses each exactly once. If a start room is provided, we
attempt to start there when valid; otherwise we pick an odd-degree vertex or
any vertex with degree > 0.

Usage:
  python euler_path.py --input example/map-aleph-cegis.json
  python euler_path.py --input example/map-aleph-cegis.json --start-room 0

Outputs a JSON object with the edge order and the implied room sequence.
"""
from __future__ import annotations

import argparse
import json
from collections import defaultdict, deque
from typing import Dict, List, Optional, Tuple


def _endpoints_of_edge(conn: dict) -> Tuple[int, int]:
    a = int(conn["from"]["room"])  # rooms are ints in JSON
    b = int(conn["to"]["room"])
    return a, b


def _build_multigraph(
    connections: List[dict],
) -> Tuple[Dict[int, List[Tuple[int, int]]], Dict[int, int]]:
    """Return (adj, degree) for an undirected multigraph on rooms.

    - adj[u] holds a list of (v, edge_index) entries; duplicates allowed.
    - degree[u] counts incident edges with multiplicity (loops +2).
    """
    adj: Dict[int, List[Tuple[int, int]]] = defaultdict(list)
    degree: Dict[int, int] = defaultdict(int)
    for idx, conn in enumerate(connections):
        a, b = _endpoints_of_edge(conn)
        adj[a].append((b, idx))
        adj[b].append((a, idx))
        if a == b:
            degree[a] += 2
        else:
            degree[a] += 1
            degree[b] += 1
    return adj, degree


def _choose_start_room(
    degree: Dict[int, int], start_pref: Optional[int]
) -> Optional[int]:
    """Pick a start room: strictly prefer start_pref if it has deg>0; else any with deg>0."""
    if start_pref is not None and degree.get(start_pref, 0) > 0:
        return start_pref
    # fallback to any node with degree>0
    for u, d in degree.items():
        if d > 0:
            return u
    return None


def euler_path(map_json: dict, start_room: Optional[int] = None) -> List[int]:
    """Return a walk covering all edges at least once (edge indices).

    - If the graph is Eulerian (0 or 2 odd nodes), returns a standard Euler trail
      that uses each edge exactly once.
    - Otherwise, returns a route that may duplicate edges as needed to reach
      remaining unused edges (simple doubling heuristic using shortest-path hops).
    """
    connections = list(map_json.get("connections", []))
    if not connections:
        raise ValueError("Graph has no edges")

    # Build basic structures
    adj, degree = _build_multigraph(connections)
    M = len(connections)

    # Fast path: Eulerian case -> exact trail with Hierholzer
    odd = [u for u, d in degree.items() if d % 2 == 1]
    preferred = start_room if start_room is not None else map_json.get("startingRoom")
    if len(odd) in (0, 2):
        if preferred is None:
            raise ValueError("start_room must be provided (or in map) to anchor start")
        s_pref = int(preferred)
        if degree.get(s_pref, 0) <= 0:
            raise ValueError("Preferred start room has no incident edges")

        # Build optional prefix to reach a valid Euler start when there are 2 odd nodes
        prefix: List[int] = []
        if len(odd) == 2 and s_pref not in odd:
            # BFS path from s_pref to the nearest odd-degree node
            targets = set(odd)
            q = deque([s_pref])
            prev: Dict[int, Tuple[int, int]] = {s_pref: (-1, -1)}
            dest: Optional[int] = None
            while q:
                u = q.popleft()
                for v, ei in adj.get(u, []):
                    if v in prev:
                        continue
                    prev[v] = (u, ei)
                    if v in targets:
                        dest = v
                        q.clear()
                        break
                    q.append(v)
            if dest is None:
                raise ValueError("No path from start_room to an odd-degree node")
            path_edges: List[int] = []
            cur = dest
            while True:
                p, ei = prev[cur]
                if p == -1:
                    break
                path_edges.append(ei)
                cur = p
            path_edges.reverse()
            prefix = path_edges
            s = dest
        else:
            s = s_pref

        # Hierholzer from s
        used: Dict[int, bool] = {}
        adj_copy: Dict[int, List[Tuple[int, int]]] = {
            u: list(lst) for u, lst in adj.items()
        }
        stack_nodes: List[int] = [s]
        stack_edges: List[int] = []
        trail_rev: List[int] = []
        while stack_nodes:
            u = stack_nodes[-1]
            lst = adj_copy.get(u, [])
            while lst and used.get(lst[-1][1], False):
                lst.pop()
            if lst:
                v, ei = lst.pop()
                if used.get(ei, False):
                    continue
                used[ei] = True
                stack_nodes.append(v)
                stack_edges.append(ei)
                adj_copy[u] = lst
            else:
                stack_nodes.pop()
                if stack_edges:
                    trail_rev.append(stack_edges.pop())
        trail = list(reversed(trail_rev))
        if len(trail) != M:
            # Graph disconnected
            raise ValueError(
                f"Graph appears disconnected: covered {len(trail)}/{M} edges"
            )
        return prefix + trail

    # Non-Eulerian: cover all edges with possible duplication.
    # Prepare adjacency usable for BFS and greedy stepping.
    adj_list: Dict[int, List[Tuple[int, int]]] = {
        u: list(lst) for u, lst in adj.items()
    }
    # Helper to check if a node has any unused incident edge
    unused = set(range(M))

    def has_unused(u: int) -> bool:
        return any(ei in unused for _, ei in adj_list.get(u, []))

    # BFS to nearest node that still has unused edges. Returns list of (next_room, edge_idx).
    def bfs_to_unused(start: int) -> Optional[List[Tuple[int, int]]]:
        q = deque([start])
        prev: Dict[int, Tuple[int, int]] = {
            start: (-1, -1)
        }  # room -> (parent_room, via_edge)
        target: Optional[int] = None
        while q:
            u = q.popleft()
            if u != start and has_unused(u):
                target = u
                break
            for v, ei in adj_list.get(u, []):
                if v in prev:
                    continue
                prev[v] = (u, ei)
                q.append(v)
        if target is None:
            return None
        # Reconstruct path as sequence of (room, edge) steps from start -> target
        path_edges: List[Tuple[int, int]] = []
        cur = target
        while True:
            p, ei = prev[cur]
            if p == -1:
                break
            path_edges.append((cur, ei))
            cur = p
        path_edges.reverse()
        return path_edges

    # Decide start node: strictly prefer preferred if possible
    s = _choose_start_room(degree, int(preferred) if preferred is not None else None)
    if s is None:
        raise ValueError("No valid start room (graph may be edgeless)")

    route: List[int] = []
    cur = s
    # Main loop: keep consuming unused edges; when stuck, walk via BFS path (duplicating edges) to nearest unused.
    while unused:
        # Greedily traverse unused edges while available from current room
        progressed = False
        for v, ei in adj_list.get(cur, []):
            if ei in unused:
                route.append(ei)
                unused.discard(ei)
                cur = v
                progressed = True
                break
        if progressed:
            continue
        # No unused incident edge here; find a path to somewhere that has one
        bridge = bfs_to_unused(cur)
        if bridge is None:
            # Unused edges remain but unreachable => disconnected graph with multiple components
            raise ValueError(
                "Graph has multiple components with edges; cannot form a single continuous route"
            )
        for nxt_room, ei in bridge:
            route.append(ei)
            # If this bridging traverses an unused edge, count it now
            unused.discard(ei)
            cur = nxt_room
    return route


def rooms_from_edges(
    map_json: dict, edges: List[int], start_room: Optional[int]
) -> List[int]:
    """Derive the room sequence from an edge order.

    Returns a list of rooms of length len(edges)+1.
    """
    cons = map_json["connections"]
    # Start from the provided start_room when possible
    adj, degree = _build_multigraph(cons)
    preferred = start_room if start_room is not None else map_json.get("startingRoom")
    actual_start = _choose_start_room(
        degree, int(preferred) if preferred is not None else None
    )
    if actual_start is None:
        raise ValueError("No valid start room (graph may be edgeless)")
    rooms: List[int] = [int(actual_start)]
    for ei in edges:
        a, b = _endpoints_of_edge(cons[ei])
        cur = rooms[-1]
        if cur == a:
            rooms.append(b)
        elif cur == b:
            rooms.append(a)
        else:
            raise ValueError(
                f"Edge {ei} does not continue the path from room {cur} (endpoints {a},{b})"
            )
    return rooms


def doors_from_edges(
    map_json: dict, edges: List[int], start_room: Optional[int]
) -> List[int]:
    """Derive the door sequence from an edge order.

    Returns a list of doors of length len(edges).
    """
    cons = map_json["connections"]
    # Start from the provided start_room when possible
    adj, degree = _build_multigraph(cons)
    preferred = start_room if start_room is not None else map_json.get("startingRoom")
    actual_start = _choose_start_room(
        degree, int(preferred) if preferred is not None else None
    )
    if actual_start is None:
        raise ValueError("No valid start room (graph may be edgeless)")
    doors: List[int] = []
    cur = actual_start
    for ei in edges:
        conn = cons[ei]
        a, b = _endpoints_of_edge(conn)
        if cur == a:
            doors.append(int(conn["from"]["door"]))
            cur = b
        elif cur == b:
            doors.append(int(conn["to"]["door"]))
            cur = a
        else:
            raise ValueError(
                f"Edge {ei} does not continue the path from room {cur} (endpoints {a},{b})"
            )
    return doors


def main():
    ap = argparse.ArgumentParser(description="Compute Euler path for a map graph JSON")
    ap.add_argument(
        "--input", required=True, help="Input map JSON path (like example/map-*.json)"
    )
    ap.add_argument(
        "--start-room", type=int, default=None, help="Preferred start room (optional)"
    )
    ap.add_argument(
        "--output",
        default=None,
        help="Write JSON result to this path (default: stdout)",
    )
    args = ap.parse_args()

    with open(args.input, "r", encoding="utf-8") as f:
        m = json.load(f)

    try:
        edges = euler_path(m, start_room=args.start_room)
        rooms = rooms_from_edges(m, edges, args.start_room or m.get("startingRoom"))
    except ValueError as e:
        out = {"error": str(e)}
    else:
        out = {"edges": edges, "rooms": rooms}

    s = json.dumps(out, ensure_ascii=False, indent=2)
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(s)
    else:
        print(s)


if __name__ == "__main__":
    main()
