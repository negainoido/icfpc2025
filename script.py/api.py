#!/usr/bin/env python3
"""
ICFPコンテスト2025 - エディフィキウム図書館マッピングツール
register以外のすべてのプロトコルエンドポイント（select, explore, guess）用CLI
"""

import json
import os
import random
import sys
from typing import Any

import click
import requests


class API:
    def __init__(self, team_id: str, base_url: str):
        self.team_id = team_id
        self.base_url = base_url

    def make_request(self, endpoint: str, data: dict[str, Any]) -> dict[str, Any]:
        """APIリクエストを送信し、レスポンスを返す"""
        url = f"{self.base_url}{endpoint}"
        try:
            response = requests.post(url, json=data)
            response.raise_for_status()
            return response.json()
        except requests.exceptions.RequestException as e:
            click.echo(f"エラー: {e}", err=True)
            sys.exit(1)

    def select(self, problem_name: str) -> dict[str, Any]:
        data = {"id": self.team_id, "problemName": problem_name}
        return self.make_request("/select", data)

    def explore(self, plans: list[str]):
        data = {"id": self.team_id, "plans": plans}
        result = self.make_request("/explore", data)
        return result

    def guess(self, map_data: dict[str, Any]) -> dict[str, Any]:
        data = {"id": self.team_id, "map": map_data}
        return self.make_request("/guess", data)


TEAM_ID = os.environ.get("TEAM_ID")
assert TEAM_ID, "環境変数TEAM_IDを設定して"
BASE_URL = os.environ.get(
    "API_HOST", "https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com"
)
print("Using HOST:", BASE_URL)
api = API(TEAM_ID, BASE_URL)


@click.group()
def cli():
    """ICFPコンテスト2025 エディフィキウム図書館マッピングツール"""
    pass


@cli.command()
@click.argument("problem_name")
def select(problem_name: str):
    """問題を選択する

    PROBLEM_NAME: 選択する問題名

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
    """
    click.echo(f"問題 '{problem_name}' を選択中...")
    result = api.select(problem_name)
    click.echo(f"✓ 問題が選択されました: {result['problemName']}")


@cli.command()
@click.argument("plans", nargs=-1, required=True)
def explore(plans: tuple):
    """エディフィキウムを探検する

    PLANS: ルートプラン（0-5の数字の文字列）を1つ以上指定

    \b
    例:
      python api.py explore "0" "12" "345"
    """
    click.echo(f"{len(plans)}個のルートプランで探検中...")
    result = api.explore(list(plans))

    click.echo(f"✓ 探検完了! 遠征回数: {result['queryCount']}")
    click.echo("\n結果:")
    for _, (plan, observations) in enumerate(zip(plans, result["results"])):
        click.echo(f"  プラン '{plan}': {observations}")

    json_output = {"plans": list(plans), "results": result["results"]}
    click.echo("\n--- smt-guessor friendly output ---")
    click.echo(json.dumps(json_output, ensure_ascii=False))


@cli.command()
@click.argument("N", type=int)
def solve(n: int):
    graph: list[list[int | None]] = [[None] * 6 for _ in range(n)]
    graph_labels = [None for _ in range(n)]
    salt = "".join([random.choice("012345") for _ in range(n * 8)])
    salts = [
        str(i) + "".join([random.choice("012345") for _ in range(5)]) for i in range(2)
    ]
    results = api.explore(salts)
    labels2node: dict[tuple[Any, ...], int] = {}
    labels_key = []
    for i in range(len(salts)):
        labels_key.append(tuple(results["results"][i][-len(salt) - 1 :]))

    labels2node[tuple(labels_key)] = 0
    graph_labels[0] = results["results"][0][0]

    while True:
        q = [(0, "")]
        visited = set()
        plans: list[tuple[tuple[int, int], str]] = []
        while q:
            current, path = q[0]
            q = q[1:]
            if current in visited:
                continue
            visited.add(current)

            for i in range(6):
                next_room = graph[current][i]
                if next_room is not None:
                    q.append((next_room, path + str(i)))
                    continue
                for salt in salts:
                    plans.append(((current, i), path + str(i) + salt))
        if not plans:
            break

        result = api.explore([plan[1] for plan in plans])
        print("plans", plans)
        print("result", result)

        for i in range(len(plans) // len(salts)):
            labels_key = []
            for j in range(len(salts)):
                labels_key.append(
                    tuple(result["results"][i * len(salts) + j][-len(salt) - 1 :])
                )
            labels_key = tuple(labels_key)
            if labels_key not in labels2node:
                labels2node[labels_key] = len(labels2node)
            node, e = plans[i * len(salts)][0]
            graph[node][e] = labels2node[labels_key]
            graph_labels[node] = result["results"][i * len(salts)][-len(salt) - 2]

        print("graph", graph)
        print("graph_labels", graph_labels)

    map_data = {
        "rooms": graph_labels,
        "startingRoom": 0,
        "connections": [],
    }
    used_edge = set()

    for i in range(n):
        for j in range(6):
            if (i, j) in used_edge:
                continue

            to = graph[i][j]
            for k in range(6):
                if to is None:
                    click.echo("❌ グラフが不完全")
                    return
                from_node = graph[to][k]
                if from_node != i:
                    continue
                if (to, k) in used_edge:
                    continue
                used_edge.add((i, j))
                used_edge.add((to, k))
                map_data["connections"].append(
                    {
                        "from": {"room": i, "door": j},
                        "to": {"room": to, "door": k},
                    }
                )
                break

    print(json.dumps(map_data, ensure_ascii=False))
    result = api.guess(map_data)
    print(result)

    if result["correct"]:
        click.echo("🎉 正解! 地図が正しく提出されました!")
    else:
        click.echo("❌ 不正解。地図が間違っています。")
        click.echo("注意: 問題は選択解除されました。新しい問題を選択してください。")


@cli.command()
@click.argument("map_file", type=click.File("r"))
def guess(map_file):
    """地図を提出する

    MAP_FILE: 地図データのJSONファイル

    \b
    地図ファイルの形式:
      {
        "rooms": [0, 1, 2, ...],
        "startingRoom": 0,
        "connections": [
          {"from": {"room": 0, "door": 0}, "to": {"room": 1, "door": 3}},
          ...
        ]
      }
    """
    try:
        map_data = json.load(map_file)
    except json.JSONDecodeError as e:
        click.echo(f"エラー: 地図ファイルのJSONが無効です: {e}", err=True)
        sys.exit(1)

    # 必須フィールドの検証
    required_fields = ["rooms", "startingRoom", "connections"]
    for field in required_fields:
        if field not in map_data:
            click.echo(
                f"エラー: 地図データに必須フィールド '{field}' がありません", err=True
            )
            sys.exit(1)

    click.echo("地図を提出中...")
    result = api.guess(map_data)

    if result["correct"]:
        click.echo("🎉 正解! 地図が正しく提出されました!")
    else:
        click.echo("❌ 不正解。地図が間違っています。")
        click.echo("注意: 問題は選択解除されました。新しい問題を選択してください。")


@cli.command()
@click.option(
    "--rooms", "-r", multiple=True, type=int, help="部屋のラベル（2ビット整数）"
)
@click.option(
    "--starting-room",
    "-s",
    type=int,
    default=0,
    help="開始部屋のインデックス（デフォルト: 0）",
)
@click.option(
    "--connection",
    "-c",
    multiple=True,
    help="接続の指定（形式: from_room,from_door,to_room,to_door）",
)
def guess_inline(rooms: tuple, starting_room: int, connection: tuple):
    """コマンドラインで直接地図を指定して提出する

    \b
    例:
      python api.py guess-inline -r 0 -r 1 -r 2 -s 0 -c "0,0,1,3" -c "1,1,2,2"
    """
    if not rooms:
        click.echo(
            "エラー: 少なくとも1つの部屋を指定してください（-r オプション）", err=True
        )
        sys.exit(1)

    connections = []
    for conn_str in connection:
        try:
            parts = conn_str.split(",")
            if len(parts) != 4:
                raise ValueError("接続は4つの値が必要です")

            from_room, from_door, to_room, to_door = map(int, parts)
            connections.append(
                {
                    "from": {"room": from_room, "door": from_door},
                    "to": {"room": to_room, "door": to_door},
                }
            )
        except ValueError as e:
            click.echo(f"エラー: 接続の形式が無効です '{conn_str}': {e}", err=True)
            sys.exit(1)

    click.echo("地図を提出中...")
    map_data = {
        "rooms": list(rooms),
        "startingRoom": starting_room,
        "connections": connections,
    }
    result = api.guess(map_data)

    if result["correct"]:
        click.echo("🎉 正解! 地図が正しく提出されました!")
    else:
        click.echo("❌ 不正解。地図が間違っています。")
        click.echo("注意: 問題は選択解除されました。新しい問題を選択してください。")


@cli.command()
def example():
    """使用例を表示する"""
    click.echo("=== ICFPコンテスト2025 エディフィキウムツール 使用例 ===\n")

    click.echo("0. 環境変数は TEAM_ID に設定する")
    click.echo("1. 問題を選択:")
    click.echo("   python main.py select probatio\n")

    click.echo("2. 探検を実行:")
    click.echo('   python main.py explore "0" "12" "345"\n')

    click.echo("3. 地図ファイルから提出:")
    click.echo("   python main.py guess map.json\n")

    click.echo("4. コマンドラインから直接提出:")
    click.echo(
        '   python main.py guess-inline -r 0 -r 1 -r 2 -s 0 -c "0,0,1,3" -c "1,1,2,2"\n'
    )

    click.echo("地図ファイル（map.json）の例:")
    example_map = {
        "rooms": [0, 1, 2],
        "startingRoom": 0,
        "connections": [
            {"from": {"room": 0, "door": 0}, "to": {"room": 1, "door": 3}},
            {"from": {"room": 1, "door": 1}, "to": {"room": 2, "door": 2}},
        ],
    }
    click.echo(json.dumps(example_map, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    cli()
