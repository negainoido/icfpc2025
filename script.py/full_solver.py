#!/bin/bash python

import api

import click

PROBLEM_SIZES = {
    "probatio": 3,
    "primus": 6,
    "secundus": 12,
    "tertius": 18,
    "quartus": 24,
    "quintus": 30,
    "aleph": 12,
    "beth": 24,
    "gimel": 36,
    "daleth": 48,
    "he": 60,
    "vau": 18,
    "zain": 36,
    "hhet": 54,
    "teth": 72,
    "iod": 90,
}


class Graph:
    def __init__(self, N):
        self.N = N
        self.graph = [[None] * 6 for _ in range(N)]
        self.reverse_door = [[None] * 6 for _ in range(N)]
        self.graph_labels = [None for _ in range(N)]
        self.visited_node_count = 0
        self.labels_to_node = [set() for _ in range(4)]
        self.reachable = {0}

    def add_node(self, label: int) -> int:
        self.graph_labels[self.visited_node_count] = label
        self.visited_node_count += 1
        self.labels_to_node[label].add(self.visited_node_count - 1)
        return self.visited_node_count - 1

    def add_edge(self, u: int, v: int, door: int, reverse_door: int | None = None):
        self.graph[u][door] = v
        self.reverse_door[u][door] = reverse_door
        if reverse_door is not None:
            self.graph[v][reverse_door] = u
            self.reverse_door[v][reverse_door] = door

    def get_path(self, u: int, v: int) -> str | None:
        queue = [(u, "")]
        visited = set()
        while queue:
            current, path = queue[0]
            queue = queue[1:]
            if current == v:
                return path
            if current in visited:
                continue
            visited.add(current)
            for door in range(6):
                if self.graph[current][door] is not None:
                    next_node = self.graph[current][door]
                    queue.append((next_node, path + str(door)))

        return None

    def get_label(self, path: str) -> int:
        result = api.api.explore([path])
        return result["results"][0][-1]

    def get_node_label(self, node_id: int) -> int:
        if self.graph_labels[node_id] is None:
            path = self.get_path(0, node_id)
            result = api.api.explore([path])
            click.echo(result)
            self.graph_labels[node_id] = result["results"][0][-1]

        return self.graph_labels[node_id]

    def get_unknown_edge(self) -> tuple[int, int] | None:
        for v in self.reachable:
            for i in range(6):
                if self.graph[v][i] is None:
                    return v, i
        return None

    def get_surround_labels(self, path: str) -> list:
        results = api.api.explore([path + str(i) for i in range(6)])["results"]
        return [result[-1] for result in results]

    def check_one_edge(self) -> bool:
        v_e = self.get_unknown_edge()
        if v_e is None:
            return False
        v, e = v_e

        label_v = self.get_node_label(v)
        path_0_v = self.get_path(0, v)
        label_v_e = self.get_label(path_0_v + str(e))
        surround_labels_v_e = self.get_surround_labels(path_0_v + str(e))

        for i, label_v_e_i in enumerate(surround_labels_v_e):
            if label_v_e_i != label_v:
                continue

            label_v1 = (label_v + 1) % 4
            path_0_v_e_i = path_0_v + "[" + str(label_v1) + "]" + str(e) + str(i)
            label_0_v_e_i = self.get_label(path_0_v_e_i)
            if label_0_v_e_i == label_v1:
                reverse_door = i  # This is the door from the neighbor back to v
                break

        label_v_e1 = (label_v_e + 1) % 4
        path_0_v_e_reverse_door = (
            path_0_v + str(e) + "[" + str(label_v_e1) + "]" + str(reverse_door)
        )

        for r in self.reachable:
            if self.get_node_label(r) != label_v_e:
                continue
            # Found a node 'r' that has the same label as the neighbor 'v_e'
            path_v_r = self.get_path(v, r)
            label_r = self.get_label(path_0_v_e_reverse_door + path_v_r)
            if label_r == label_v_e1:
                self.add_edge(v, r, e, reverse_door)
                return True

        self.add_node(label_v_e)
        new_node_id = self.visited_node_count - 1
        self.add_edge(v, new_node_id, e, reverse_door)
        self.reachable.add(new_node_id)

        return True

    def get_map_data(self) -> dict:
        map_data = {
            "rooms": self.graph_labels,
            "startingRoom": 0,
            "connections": [],
        }
        used_edge = set()
        for u in range(self.N):
            for door in range(6):
                v = self.graph[u][door]
                reverse_door = self.reverse_door[u][door]
                if (u, door) not in used_edge and (v, reverse_door) not in used_edge:
                    map_data["connections"].append(
                        {
                            "from": {"room": u, "door": door},
                            "to": {"room": v, "door": reverse_door},
                        }
                    )
                    used_edge.add((u, door))
                    used_edge.add((v, reverse_door))
        return map_data


@click.group()
def cli():
    pass


@cli.command()
@click.argument("problem_name")
def solve(problem_name: str):
    """指定された問題の地図を自動的に解く

    PROBLEM_NAME: 解く問題名

    利用可能な問題:

    \b
      Problem     Size
      ----------- ----
      probatio       3
      primus         6
      secundus      12
      tertius       18
      quartus       24
      quintus       30
      aleph         12
      beth          24
      gimel         36
      daleth        48
      he            60
      vau           18
      zain          36
      hhet          54
      teth          72
      iod           90
    """
    click.echo(f"問題 '{problem_name}' を選択中...")
    api.api.select(problem_name)
    click.echo(f"✓ 問題が選択されました: {problem_name}")

    if problem_name not in PROBLEM_SIZES:
        click.secho(
            f"エラー: 問題 '{problem_name}' のサイズが不明です。", err=True, fg="red"
        )
        return

    N = PROBLEM_SIZES[problem_name]
    click.echo(f"問題サイズ: {N}")
    graph = Graph(N)
    graph.add_node(graph.get_node_label(0))

    while True:
        if not graph.check_one_edge():
            break
        print(graph.graph)
        print(graph.graph_labels)

    print(api.api.guess(graph.get_map_data()))


if __name__ == "__main__":
    cli()
