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

"""

辺を一本ずつ探索してグラフを構築する
最初は到達可能なノードは0番ノード、そのラベルも既知

到達可能なノードから伸びている未知の辺を一本取ってくる
* 未知の辺とその逆向きのドアを見つける
* その辺の先のノードが既知のノードか未知のノードかを判定する

到達可能なノード: V
未知か既知かわからない隣のノード: X
そこから伸びている未知の辺を: e
Vのラベル label_v

V - e -> X - 0 -> Y0
V - e -> X - 1 -> Y1
...
V - e -> X - 5 -> Y5

Vを (label_v  + 1) % 4で塗ってもう一回Xの隣接するノードのラベルを確認して、
Yiのラベルが(label_v + 1) % 4と一致するものを探す


XからVに戻る辺: reverse_doorが分かる

今度はXが既知のノードなのか、未知のノードなのかを判別する
既知のノードのうちXと同じラベルを持つものを探す

ZがXと同じラベルを持っていたとする。Zは既知のノード

V - e -> X (Xのラベルを(label_x + 1)%4にする) - reverse_e -> V - - - -> Z

Zのラベルが(label_x+1)%4で塗られていたらZはXと同じ

もしそのようなZが存在しなければ、Xは未知のノード

"""

class Graph:
    def __init__(self, N):
        self.N = N
        self.M = N * 6  # Total number of possible edges
        self.graph: list[list[int | None]] = [[None] * 6 for _ in range(N)]
        self.reverse_door: list[list[int | None]] = [[None] * 6 for _ in range(N)]
        self.graph_labels: list[int | None] = [None for _ in range(N)]
        self.visited_node_count = 0
        self.reachable = {0}

    def add_new_node(self, label: int) -> int:
        self.graph_labels[self.visited_node_count] = label
        self.visited_node_count += 1
        return self.visited_node_count - 1

    def get_remaining_edges(self) -> int:
        cnt = 0
        for e in self.graph:
            cnt += e.count(None)
        return cnt

    def add_edge(self, u: int, v: int, door: int, reverse_door: int | None = None):
        self.graph[u][door] = v
        self.reverse_door[u][door] = reverse_door
        if reverse_door is not None:
            self.graph[v][reverse_door] = u
            self.reverse_door[v][reverse_door] = door

    def get_path(self, u: int, v: int) -> str:
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
                next_node = self.graph[current][door]
                if next_node is None:
                    continue
                queue.append((next_node, path + str(door)))

        assert False, "Path not found"

    def get_labels(self, path: str) -> list[int]:
        result = api.api.explore([path])
        return result["results"][0]

    def get_label(self, path: str) -> int:
        return self.get_labels(path)[-1]

    def get_labels(self, paths: list[str]) -> list[int]:
        results = api.api.explore(paths)["results"]
        return [result[-1] for result in results]

    def get_node_label(self, node_id: int) -> int:
        if self.graph_labels[node_id] is None:
            path = self.get_path(0, node_id)
            self.graph_labels[node_id] = self.get_label(path)

        label = self.graph_labels[node_id]
        assert label is not None, "GetNode label is None"
        return label

    def get_unknown_edge(self) -> tuple[int, int] | None:
        for v in self.reachable:
            for i in range(6):
                if self.graph[v][i] is None:
                    return v, i
        return None

    # pathの先のノードと、その一つ先のノードのラベルを得る
    def get_surround_labels(self, path: str) -> tuple[int, list]:
        results = api.api.explore([path + str(i) for i in range(6)])["results"]
        # 対象ノードのラベル
        label_v = results[0][-2]
        return (label_v, [result[-1] for result in results])

    def check_one_edge(self) -> bool:
        v_e = self.get_unknown_edge()
        if v_e is None:
            return False
        v, e = v_e

        label_v = self.get_node_label(v)
        path_0_v = self.get_path(0, v)
        [label_v_e, surround_labels_v_e] = self.get_surround_labels(path_0_v + str(e))

        # 逆向きの辺を探す
        reverse_doors = set()
        reverse_door_plans = [] # batchで投げられるように配列をつくる
        for i, label_v_e_i in enumerate(surround_labels_v_e):
            if label_v_e_i != label_v:
                continue

            label_v1 = (label_v + 1) % 4
            path_0_v_e_i = path_0_v + "[" + str(label_v1) + "]" + str(e) + str(i)
            reverse_door_plans.append([path_0_v_e_i, i, label_v1])

        reverse_door_plan_results = self.get_labels([plan[0] for plan in reverse_door_plans])
        for i, label in enumerate(reverse_door_plan_results):
            if label == reverse_door_plans[i][2]:
                reverse_doors.add(reverse_door_plans[i][1]) # This is the door from the neighbor back to v

        assert reverse_doors, "Reverse door not found"

        if all(self.get_node_label(r) != label_v_e for r in self.reachable):
            new_node_id = self.add_new_node(label_v_e)
            self.add_edge(v, new_node_id, e, list(reverse_doors)[0])
            self.reachable.add(new_node_id)
            return True

        label_v_e1 = (label_v_e + 1) % 4
        path_0_v_e_reverse_door = (
            path_0_v
            + str(e)
            + "["
            + str(label_v_e1)
            + "]"
            + str(list(reverse_doors)[0])
        )

        visited = set()

        def dfs(v: int) -> str:
            if v in visited:
                return ""
            visited.add(v)
            ret = ""
            for i in range(6):
                if self.graph[v][i] is None:
                    continue
                ret += str(i)
                ret += dfs(self.graph[v][i])
                ret += str(self.reverse_door[v][i])
            return ret

        visit_all_path = dfs(v)
        v_e_reverse_door_visit_all_path = path_0_v_e_reverse_door + visit_all_path

        visit_all_path_labels = self.get_labels(v_e_reverse_door_visit_all_path)[
            -len(visit_all_path) - 1 :
        ]

        visit_all_path_doors = list(
            map(
                int,
                list(visit_all_path),
            )
        )

        label_r = visit_all_path_labels[0]
        if label_r == label_v_e1:
            for reverse_door in reverse_doors:
                if self.graph[v][reverse_door] is not None:
                    continue

                self.add_edge(v, v, e, reverse_door)
                return True
            assert False, "Reverse door not found for v"

        current_v = v
        for idx, door in enumerate(visit_all_path_doors):
            current_v = self.graph[current_v][door]
            if self.get_node_label(current_v) != label_v_e:
                continue

            label_r = visit_all_path_labels[idx + 1]
            if label_r == label_v_e1:
                for reverse_door in reverse_doors:
                    if self.graph[current_v][reverse_door] is not None:
                        continue

                    self.add_edge(v, current_v, e, reverse_door)
                    return True

        assert False, "not Reachable nor unreachable?"

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
    graph.add_new_node(graph.get_node_label(0))

    edge_num = 0
    while True:
        edge_num += 1
        click.echo(f"辺数: {edge_num}, 残りの辺数: {graph.get_remaining_edges()}")
        if not graph.check_one_edge():
            break

    print(api.api.guess(graph.get_map_data()))

if __name__ == "__main__":
    cli()
