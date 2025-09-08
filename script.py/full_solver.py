#!/bin/bash python


import click

import api
from graph import Graph

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
