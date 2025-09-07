#!/usr/bin/env python3
"""
ICFPコンテスト2025 - エディフィキウム図書館マッピングツール
register以外のすべてのプロトコルエンドポイント（select, explore, guess）用CLI
"""

import json
import os
import random
import sys
import time
from typing import Any

import click
import requests
from dotenv import load_dotenv


class API:
    @staticmethod
    def build():
        """環境変数からAPIクライアントを構築する

        - TEAM_ID, API_HOST, USER が設定されていれば本番直接/ローカルモックサーバに接続
        - CLIENT_ID, CLIENT_SECRET, USER が設定されていればgarasubo.com経由で接続
        - どちらも設定されていなければエラー終了
        """
        load_dotenv()
        TEAM_ID = os.environ.get("TEAM_ID")
        API_HOST = os.environ.get("API_HOST")
        CLIENT_ID = os.environ.get("CLIENT_ID")
        CLIENT_SECRET = os.environ.get("CLIENT_SECRET")
        USER_NAME = os.environ.get("USER")

        if TEAM_ID and API_HOST:
            print(f"Using direct API access to {API_HOST} as {TEAM_ID}")
            api = API(API_HOST, TEAM_ID, None, None, None)
        elif CLIENT_ID and CLIENT_SECRET and USER_NAME:
            print(f"Using garasubo.com API access as {CLIENT_ID}")
            api = API(None, None, CLIENT_ID, CLIENT_SECRET, USER_NAME)
        else:
            print(
                "Error: Set {TEAM_ID and API_HOST} for prod/local , or {CLIENT_ID, CLIENT_SECRET and USER} for garasubo.com"
            )
            sys.exit(1)

        return api

    def __init__(
        self,
        base_url: str | None,
        team_id: str | None,
        client_id: str | None,
        client_secret: str | None,
        user_name: str | None,
    ):
        """API client

        Parameters
        ----------
        base_url
            本番直接/ローカルモックサーバに必要
        team_id
            本番直接/ローカルモックサーバに必要
        client_id
            garasubo.com に必要
        client_secret
            garasubo.com に必要
        user_name
            garasubo.com のときに使う
        """
        self.base_url = base_url or "https://negainoido.garasubo.com/api"
        self.team_id = team_id
        self.client_id = client_id
        self.client_secret = client_secret
        self.user_name = user_name

    def make_request(
        self,
        endpoint: str,
        data: dict[str, Any],
        max_retries: int = 10,
    ) -> dict[str, Any] | None:
        """APIリクエストを送信し、レスポンスを返す

        500系エラーに限って max_retries 回までリトライする
        """
        url = f"{self.base_url}{endpoint}"
        headers = {
            "CF-Access-Client-Id": self.client_id,
            "CF-Access-Client-Secret": self.client_secret,
        }
        data = {key: val for key, val in data.items() if val}
        for i_try in range(max_retries):
            try:
                response = requests.post(url, json=data, headers=headers)
                response.raise_for_status()
                return response.json()
            except requests.exceptions.RequestException as e:
                click.secho(e, err=True, fg="red")
                if e.response is not None:
                    click.secho(f"{e.response.text}", err=True, fg="red")
                    if e.response.status_code >= 500:
                        click.secho(
                            f"Retrying... [{i_try}/{max_retries}]", err=True, fg="yellow"
                        )
                        time.sleep(0.1 * (1.6**i_try))
                        continue
                sys.exit(1)

    def select(self, problem_name: str) -> dict[str, Any]:
        data = {
            "id": self.team_id,
            "user_name": self.user_name,
            "problemName": problem_name,
        }
        result = self.make_request("/select", data)
        assert result is not None, "Request failed"
        if "session_id" in result:
            self.session_id = result["session_id"]
            print(f"SessionId: {self.session_id}")
        return result

    def explore(self, plans: list[str]):
        data = {"id": self.team_id, "user_name": self.user_name, "plans": plans}
        result = self.make_request("/explore", data)
        assert result is not None, "Request failed"
        return result

    def guess(self, map_data: dict[str, Any]) -> dict[str, Any]:
        data = {"id": self.team_id, "user_name": self.user_name, "map": map_data}
        result = self.make_request("/guess", data)
        assert result is not None, "Request failed"
        return result

    def make_get_request(self, endpoint: str) -> dict[str, Any]:
        """GETリクエストを送信し、レスポンスを返す"""
        url = f"{self.base_url}{endpoint}"
        try:
            headers = {
                "CF-Access-Client-Id": self.client_id,
                "CF-Access-Client-Secret": self.client_secret,
            }
            response = requests.get(url, headers=headers)
            response.raise_for_status()
            return response.json()
        except requests.exceptions.RequestException as e:
            click.secho(e, err=True, fg="red")
            sys.exit(1)

    def make_put_request(self, endpoint: str) -> bool:
        """PUTリクエストを送信し、成功可否を返す"""
        url = f"{self.base_url}{endpoint}"
        try:
            headers = {
                "CF-Access-Client-Id": self.client_id,
                "CF-Access-Client-Secret": self.client_secret,
            }
            response = requests.put(url, headers=headers)
            response.raise_for_status()
            return True
        except requests.exceptions.RequestException as e:
            click.secho(e, err=True, fg="red")
            return False

    def get_sessions(self) -> dict[str, Any]:
        """全セッション一覧を取得"""
        return self.make_get_request("/sessions")

    def get_current_session(self) -> dict[str, Any] | None:
        """現在のアクティブセッション情報を取得"""
        return self.make_get_request("/sessions/current")

    def get_session_detail(self, session_id: str) -> dict[str, Any]:
        """特定セッションの詳細情報とAPIログ履歴を取得"""
        return self.make_get_request(f"/sessions/{session_id}")

    def abort_session(self, session_id: str) -> bool:
        """セッションを強制中止"""
        return self.make_put_request(f"/sessions/{session_id}/abort")


api = API.build()


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
    salts = [
        str(i) + "".join([random.choice("012345") for _ in range(5)]) for i in range(2)
    ]
    results = api.explore(salts)
    labels2node: dict[tuple[Any, ...], int] = {}
    labels_key = []
    for i, salt in enumerate(salts):
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
def guess_inline(
    rooms: tuple,
    starting_room: int,
    connection: tuple,
):
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
def sessions():
    """全セッションの一覧を表示する"""
    result = api.get_sessions()
    click.echo("=== セッション一覧 ===")
    for session in result["sessions"]:
        status_emoji = (
            "🟢"
            if session["status"] == "active"
            else "⚪"
            if session["status"] == "completed"
            else "🔴"
        )
        user_info = f" ({session['user_name']})" if session["user_name"] else ""
        click.echo(
            f"{status_emoji} {session['session_id']} - {user_info} - {session['status']} - {session['created_at']}"
        )


@cli.command()
def session_current():
    """現在のアクティブセッション情報を表示する"""
    result = api.get_current_session()
    if result is None:
        click.echo("現在アクティブなセッションはありません")
    else:
        click.echo("=== 現在のアクティブセッション ===")
        click.echo(f"Session ID: {result['session_id']}")
        click.echo(f"User: {result['user_name'] or 'N/A'}")
        click.echo(f"Status: {result['status']}")
        click.echo(f"Created: {result['created_at']}")


@cli.command()
@click.argument("session_id")
def session_detail(session_id: str):
    """特定セッションの詳細情報とAPIログ履歴を表示する

    SESSION_ID: 詳細を表示するセッションID
    """
    result = api.get_session_detail(session_id)
    session = result["session"]
    api_logs = result["api_logs"]

    click.echo("=== セッション詳細 ===")
    click.echo(f"Session ID: {session['session_id']}")
    click.echo(f"User: {session['user_name'] or 'N/A'}")
    click.echo(f"Status: {session['status']}")
    click.echo(f"Created: {session['created_at']}")
    if session["completed_at"]:
        click.echo(f"Completed: {session['completed_at']}")

    click.echo(f"\n=== APIログ履歴 ({len(api_logs)}件) ===")
    for log in api_logs:
        status_emoji = "✅" if log["response_status"] == 200 else "❌"
        click.echo(
            f"{status_emoji} {log['endpoint']} - {log['response_status']} - {log['created_at']}"
        )
        if log["endpoint"] == "explore":
            try:
                req = json.loads(log["request_body"])
                resp = json.loads(log["response_body"])
                click.echo(f"   Plans: {req.get('plans', [])}")
                click.echo(f"   Query Count: {resp.get('queryCount', 'N/A')}")
            except Exception:
                pass


@cli.command()
@click.argument("session_id")
@click.confirmation_option(prompt="本当にこのセッションを中止しますか？")
def session_abort(session_id: str):
    """セッションを強制中止する

    SESSION_ID: 中止するセッションID
    """
    success = api.abort_session(session_id)
    if success:
        click.echo(f"✅ セッション {session_id[:8]}... を中止しました")
    else:
        click.echo(f"❌ セッション {session_id[:8]}... の中止に失敗しました")


if __name__ == "__main__":
    cli()
