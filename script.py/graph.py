import random

import api

class UnionFind:
    def __init__(self, n):
        self.parent = list(range(n))

    def find(self, i):
        if self.parent[i] == i:
            return i
        self.parent[i] = self.find(self.parent[i])
        return self.parent[i]

    def union(self, i, j):
        root_i = self.find(i)
        root_j = self.find(j)
        if root_i != root_j:
            self.parent[root_i] = root_j
            return True
        return False


class Graph:
    def __init__(self, N, plan_limit_multiplier: int = 18):
        self.N = N
        self.M = N * 6  # Total number of possible edges
        self.plan_limit = plan_limit_multiplier * N
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
        if self.graph[u][door] is not None and reverse_door is None:
            return

        self.graph[u][door] = v
        if reverse_door is not None:
            self.reverse_door[u][door] = reverse_door
            self.graph[v][reverse_door] = u
            self.reverse_door[v][reverse_door] = door
            return

        for i in range(6):
            if self.graph[v][i] is u and self.reverse_door[v][i] is None:
                self.reverse_door[u][door] = i
                self.reverse_door[v][i] = door
                return

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

    def get_multiple_labels(self, paths: list[str]) -> list[int]:
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
                if self.graph[v][i] is None or self.reverse_door[v][i] is None:
                    return v, i
        return None

    def gen_randome_walk(self):
        return [random.randint(0, 5) for _ in range(self.plan_limit)]

    def solve_random_walk(self):
        path = self.gen_randome_walk()
        labels = self.get_labels("".join(map(str, path)))
        uf = UnionFind(len(labels))
        merged = [False for _ in range(len(labels))]

        while not all(merged):
            next_path = []
            checking_label = {}
            for i, label in enumerate(labels[:-1]):
                if merged[i] or label in checking_label:
                    next_path.append((path[i], None))
                    continue
                merged[i] = True
                checking_label[label] = i
                next_path.append((path[i], (label + 1) % 4))

            next_paths_str = "".join(
                f"{d}" if l is None else f"[{l}]{d}" for d, l in next_path
            )
            next_labels = self.get_labels(next_paths_str)
            next_label_idx = 1
            for i, (d, l) in enumerate(next_path):
                if l is not None:
                    next_label_idx += 1
                original_label = labels[i + 1]
                if next_labels[next_label_idx] != original_label:
                    j = checking_label[original_label]
                    uf.union(i + 1, j)
                    merged[i + 1] = True
                    merged[j] = True

                next_label_idx += 1

        assert all(merged), f"labels: {labels}, merged: {merged}, path: {path}"

        uf_to_node = {}
        for i, p in enumerate(path):
            u = uf.find(i)
            v = uf.find(i + 1)
            if u not in uf_to_node:
                uf_to_node[u] = self.add_new_node(labels[i])
            if v not in uf_to_node:
                uf_to_node[v] = self.add_new_node(labels[i + 1])
            self.add_edge(uf_to_node[u], uf_to_node[v], p, None)

        print(self.graph)
        print(self.reverse_door)
        print(self.graph_labels)
        q = [0]
        while q:
            cv = q[0]
            q = q[1:]
            self.reachable.add(cv)
            for i in range(6):
                to = self.graph[cv][i]
                if to is None or to in self.reachable:
                    continue
                if self.reverse_door[cv][i] is None:
                    continue
                q.append(to)

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
        print(v, e)

        label_v = self.get_node_label(v)
        path_0_v = self.get_path(0, v)
        [label_v_e, surround_labels_v_e] = self.get_surround_labels(path_0_v + str(e))

        # 逆向きの辺を探す
        reverse_doors = set()
        reverse_door_plans = []  # batchで投げられるように配列をつくる
        for i, label_v_e_i in enumerate(surround_labels_v_e):
            if label_v_e_i != label_v:
                continue

            label_v1 = (label_v + 1) % 4
            path_0_v_e_i = path_0_v + "[" + str(label_v1) + "]" + str(e) + str(i)
            reverse_door_plans.append([path_0_v_e_i, i, label_v1])

        reverse_door_plan_results = self.get_multiple_labels(
            [plan[0] for plan in reverse_door_plans]
        )
        for i, label in enumerate(reverse_door_plan_results):
            if label == reverse_door_plans[i][2]:
                reverse_doors.add(
                    reverse_door_plans[i][1]
                )  # This is the door from the neighbor back to v

        assert reverse_doors, "Reverse door not found"

        if all(self.get_node_label(r) != label_v_e for r in self.reachable):
            new_node_id = self.add_new_node(label_v_e)
            self.add_edge(v, new_node_id, e, list(reverse_doors)[0])
            self.reachable.add(new_node_id)
            return True

        label_v_e1 = (label_v_e + 1) % 4
        if label_v_e1 == label_v:
            label_v_e1 = (label_v_e1 + 1) % 4

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
                nv = self.graph[v][i]
                if nv is None or nv in visited:
                    continue
                if self.reverse_door[v][i] is None:
                    continue
                ret += str(i)
                ret += dfs(nv)
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

        new_label_v = visit_all_path_labels[0]
        if new_label_v == label_v_e1:
            for reverse_door in reverse_doors:
                if self.graph[v][reverse_door] is not None:
                    continue

                self.add_edge(v, v, e, reverse_door)
                return True
            assert False, "Reverse door not found for v"

        current_v = v
        visit_node = [v]
        for idx, door in enumerate(visit_all_path_doors):
            current_v = self.graph[current_v][door]
            visit_node.append(current_v)
            if self.get_node_label(current_v) != label_v_e:
                continue

            label_r = visit_all_path_labels[idx + 1]
            if label_r == label_v_e1:
                for reverse_door in reverse_doors:
                    cv_reverse = self.graph[current_v][reverse_door]
                    cv_reverse_door = self.reverse_door[current_v][reverse_door]
                    if cv_reverse is not None and cv_reverse_door is not None:
                        continue

                    self.add_edge(v, current_v, e, reverse_door)
                    return True

                assert False, "Reverse door not found for current_v"

        new_node_id = self.add_new_node(label_v_e)
        self.add_edge(v, new_node_id, e, list(reverse_doors)[0])
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
