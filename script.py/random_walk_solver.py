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


if __name__ == "__main__":
    cli()
