#!/usr/bin/env python3
"""
ICFPコンテスト2025 - エディフィキウム図書館マッピングツール
register以外のすべてのプロトコルエンドポイント（select, explore, guess）用CLI
"""

import json
import sys
from typing import Any, Dict

import click
import requests

BASE_URL = "https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com"


def make_request(endpoint: str, data: Dict[str, Any]) -> Dict[str, Any]:
    """APIリクエストを送信し、レスポンスを返す"""
    url = f"{BASE_URL}{endpoint}"

    try:
        response = requests.post(url, json=data)
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        click.echo(f"エラー: {e}", err=True)
        sys.exit(1)


@click.group()
def cli():
    """ICFPコンテスト2025 エディフィキウム図書館マッピングツール"""
    pass


@cli.command()
@click.argument("team_id")
@click.argument("problem_name")
def select(team_id: str, problem_name: str):
    """問題を選択する

    TEAM_ID: /registerで取得したチームID
    PROBLEM_NAME: 選択する問題名（例: probatio）
    """
    data = {"id": team_id, "problemName": problem_name}

    click.echo(f"問題 '{problem_name}' を選択中...")
    result = make_request("/select", data)

    click.echo(f"✓ 問題が選択されました: {result['problemName']}")


@cli.command()
@click.argument("team_id")
@click.argument("plans", nargs=-1, required=True)
def explore(team_id: str, plans: tuple):
    """エディフィキウムを探検する

    TEAM_ID: /registerで取得したチームID
    PLANS: ルートプラン（0-5の数字の文字列）を1つ以上指定

    例: python main.py explore YOUR_ID "0" "12" "345"
    """
    data = {"id": team_id, "plans": list(plans)}

    click.echo(f"{len(plans)}個のルートプランで探検中...")
    result = make_request("/explore", data)

    click.echo(f"✓ 探検完了! 遠征回数: {result['queryCount']}")
    click.echo("\n結果:")
    for _, (plan, observations) in enumerate(zip(plans, result["results"])):
        click.echo(f"  プラン '{plan}': {observations}")


@cli.command()
@click.argument("team_id")
@click.argument("map_file", type=click.File("r"))
def guess(team_id: str, map_file):
    """地図を提出する

    TEAM_ID: /registerで取得したチームID
    MAP_FILE: 地図データのJSONファイル

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

    data = {"id": team_id, "map": map_data}

    click.echo("地図を提出中...")
    result = make_request("/guess", data)

    if result["correct"]:
        click.echo("🎉 正解! 地図が正しく提出されました!")
    else:
        click.echo("❌ 不正解。地図が間違っています。")
        click.echo("注意: 問題は選択解除されました。新しい問題を選択してください。")


@cli.command()
@click.argument("team_id")
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
def guess_inline(team_id: str, rooms: tuple, starting_room: int, connection: tuple):
    """コマンドラインで直接地図を指定して提出する

    TEAM_ID: /registerで取得したチームID

    例: python main.py guess-inline YOUR_ID -r 0 -r 1 -r 2 -s 0 -c "0,0,1,3" -c "1,1,2,2"
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

    map_data = {
        "rooms": list(rooms),
        "startingRoom": starting_room,
        "connections": connections,
    }

    data = {"id": team_id, "map": map_data}

    click.echo("地図を提出中...")
    result = make_request("/guess", data)

    if result["correct"]:
        click.echo("🎉 正解! 地図が正しく提出されました!")
    else:
        click.echo("❌ 不正解。地図が間違っています。")
        click.echo("注意: 問題は選択解除されました。新しい問題を選択してください。")


@cli.command()
def example():
    """使用例を表示する"""
    click.echo("=== ICFPコンテスト2025 エディフィキウムツール 使用例 ===\n")

    click.echo("1. 問題を選択:")
    click.echo("   python main.py select YOUR_TEAM_ID probatio\n")

    click.echo("2. 探検を実行:")
    click.echo('   python main.py explore YOUR_TEAM_ID "0" "12" "345"\n')

    click.echo("3. 地図ファイルから提出:")
    click.echo("   python main.py guess YOUR_TEAM_ID map.json\n")

    click.echo("4. コマンドラインから直接提出:")
    click.echo(
        '   python main.py guess-inline YOUR_TEAM_ID -r 0 -r 1 -r 2 -s 0 -c "0,0,1,3" -c "1,1,2,2"\n'
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
