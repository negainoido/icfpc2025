#!/usr/bin/env python3
"""
ICFPコンテスト2025 エディフィキウム図書館マッピング モックサーバー
FastAPIを使用してすべてのプロトコルを実装
"""

import logging
import random
import uuid
from dataclasses import dataclass
from typing import Dict, List, Optional

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

logger = logging.getLogger("uvicorn")
app = FastAPI(title="エディフィキウム図書館マッピング API", version="1.0.0")


# データモデル
class RegisterRequest(BaseModel):
    name: str
    pl: str
    email: str


class RegisterResponse(BaseModel):
    id: str


class SelectRequest(BaseModel):
    id: str
    problemName: str


class SelectResponse(BaseModel):
    problemName: str


class ExploreRequest(BaseModel):
    id: str
    plans: List[str]


class ExploreResponse(BaseModel):
    results: List[List[int]]
    queryCount: int


class ConnectionPoint(BaseModel):
    room: int
    door: int


class Connection(BaseModel):
    from_: ConnectionPoint = Field(alias="from")
    to: ConnectionPoint

    class Config:
        allow_population_by_field_name = True


class MapData(BaseModel):
    rooms: List[int]
    startingRoom: int
    connections: List[Connection]


class GuessRequest(BaseModel):
    id: str
    map: MapData


class GuessResponse(BaseModel):
    correct: bool


# 問題サイズの定義
PROBLEM_SIZES = {
    "probatio": 3,
    "primus": 6,
    "secundus": 12,
    "tertius": 18,
    "quartus": 24,
    "quintus": 30,
}


@dataclass
class Room:
    """部屋を表すクラス"""

    label: int  # 2ビット整数 (0-3)
    doors: Dict[int, tuple]  # door_id -> (connected_room_id, connected_door_id)


@dataclass
class Problem:
    """問題インスタンスを表すクラス"""

    name: str
    rooms: List[Room]
    starting_room: int = 0


@dataclass
class Team:
    """チーム情報を表すクラス"""

    id: str
    name: str
    pl: str
    email: str
    current_problem: Optional[Problem] = None
    query_count: int = 0


# グローバル状態
teams: Dict[str, Team] = {}


def generate_random_problem(problem_name: str) -> Problem:
    """指定された問題名に基づいてランダムな問題を生成する"""
    if problem_name not in PROBLEM_SIZES:
        raise ValueError(f"Unknown problem: {problem_name}")

    size = PROBLEM_SIZES[problem_name]

    # 各部屋にランダムな2ビットラベルを割り当て
    rooms = []
    for _ in range(size):
        label = random.randint(0, 3)  # 2ビット整数 (0-3)
        room = Room(label=label, doors={})
        rooms.append(room)

    # 無向グラフになるように接続を生成
    # 既に接続されているエッジを追跡するセット
    connected_edges = set()

    # 各部屋の各ドア（0-5）にランダムな接続を生成
    for room_id in range(size):
        for door_id in range(6):  # ドア0-5
            # 既に接続が設定されている場合はスキップ
            if (room_id, door_id) in connected_edges:
                continue

            # ランダムな接続先を選択
            target_room = random.randint(0, size - 1)
            target_door = random.randint(0, 5)

            # 既に接続されている場合は別の接続先を探す
            max_attempts = 50  # 無限ループを避けるため
            attempts = 0
            while (
                target_room,
                target_door,
            ) in connected_edges and attempts < max_attempts:
                target_room = random.randint(0, size - 1)
                target_door = random.randint(0, 5)
                attempts += 1

            # 無向エッジを作成（双方向に接続）
            rooms[room_id].doors[door_id] = (target_room, target_door)
            rooms[target_room].doors[target_door] = (room_id, door_id)

            # 接続済みエッジとして記録
            connected_edges.add((room_id, door_id))
            connected_edges.add((target_room, target_door))

    problem = Problem(name=problem_name, rooms=rooms, starting_room=0)

    # 生成された問題情報をログ出力
    print(f"\n=== 生成された問題 ({problem_name}) ===")
    print(f"部屋数: {size}")
    print(f"開始部屋: {problem.starting_room}")
    print("部屋ラベル:")
    for i, room in enumerate(rooms):
        print(f"  部屋{i}: ラベル{room.label}")
    print("\nJSON形式グラフ:")
    json_graph = generate_json_graph(problem)
    print(json_graph)
    print("=" * 50)

    # JSON形式の地図を /tmp/map.json に保存
    with open("/tmp/map.json", "w", encoding="utf-8") as f:
        f.write(json_graph)

    return problem


def generate_json_graph(problem: Problem) -> str:
    """JSON形式でグラフを生成する（guess形式と同じ）"""
    import json

    # 部屋のラベルを収集
    rooms = [room.label for room in problem.rooms]

    # 接続情報を収集（重複を避けるため）
    connections = []
    processed_edges = set()

    for room_id, room in enumerate(problem.rooms):
        for door_id, (target_room, target_door) in room.doors.items():
            # 無向グラフなので、重複を避けるため順序付きペアで管理
            edge_key = tuple(sorted([(room_id, door_id), (target_room, target_door)]))

            if edge_key not in processed_edges:
                connections.append(
                    {
                        "from": {"room": room_id, "door": door_id},
                        "to": {"room": target_room, "door": target_door},
                    }
                )
                processed_edges.add(edge_key)

    graph_data = {
        "rooms": rooms,
        "startingRoom": problem.starting_room,
        "connections": connections,
    }

    return json.dumps(graph_data, ensure_ascii=False, separators=(",", ":"))


def simulate_exploration(problem: Problem, plans: List[str]) -> List[List[int]]:
    """ルートプランを実行して観察結果を返す"""
    results = []

    for plan in plans:
        observations = []
        current_room = problem.starting_room

        # 開始部屋のラベルを記録
        observations.append(problem.rooms[current_room].label)

        # 各ドアを通過
        for door_char in plan:
            try:
                door_id = int(door_char)
                if door_id < 0 or door_id > 5:
                    raise ValueError(f"Invalid door: {door_id}")

                # 現在の部屋の指定されたドアを通過
                if door_id in problem.rooms[current_room].doors:
                    next_room, _ = problem.rooms[current_room].doors[door_id]
                    current_room = next_room
                    observations.append(problem.rooms[current_room].label)
                else:
                    # ドアが存在しない場合（通常はすべてのドアが存在するはず）
                    observations.append(problem.rooms[current_room].label)

            except ValueError:
                # 無効な文字は無視
                continue

        results.append(observations)

    return results


def maps_are_equivalent(problem: Problem, submitted_map: MapData) -> bool:
    """提出された地図が問題の地図と等価かチェック"""
    # 部屋数が一致するかチェック
    if len(submitted_map.rooms) != len(problem.rooms):
        logger.warning("部屋数が一致しない")
        return False

    # 開始部屋のラベルが一致するかチェック
    if submitted_map.startingRoom >= len(submitted_map.rooms):
        logger.warning("開始部屋のラベルが不正")
        return False

    expected_start_label = problem.rooms[problem.starting_room].label
    submitted_start_label = submitted_map.rooms[submitted_map.startingRoom]

    if expected_start_label != submitted_start_label:
        logger.warning("開始部屋のラベルが不一致")
        return False

    # 簡単な等価性チェック: 各ルートプランで同じ結果が得られるかテスト
    # より厳密な実装では、グラフ同型性をチェックする必要がある
    test_plans = [
        "0",
        "1",
        "2",
        "3",
        "4",
        "5",
        "01",
        "12",
        "23",
        "34",
        "45",
        "50",
        "123",
        "0123",
        "1234",
        "2345",
        "3450",
        "4501",
        "5012",
        "31415",
        "010101232334454545",
    ]

    for plan in test_plans:
        original_results = simulate_exploration(problem, [plan])
        submitted_results = simulate_submitted_map(submitted_map, [plan])

        if original_results != submitted_results:
            logger.warning(f"プラン '{plan}' で結果が一致しない")
            logger.warning(f"期待される結果: {original_results}")
            logger.warning(f"提出された結果: {submitted_results}")
            return False

    return True


def simulate_submitted_map(map_data: MapData, plans: List[str]) -> List[List[int]]:
    """提出された地図でルートプランをシミュレート"""
    results = []

    # 接続情報を辞書に変換（無向グラフなので双方向に設定）
    connections = {}
    for conn in map_data.connections:
        from_room = conn.from_.room
        from_door = conn.from_.door
        to_room = conn.to.room
        to_door = conn.to.door

        # from -> to の接続
        if from_room not in connections:
            connections[from_room] = {}
        connections[from_room][from_door] = to_room

        # to -> from の接続（無向グラフ）
        if to_room not in connections:
            connections[to_room] = {}
        connections[to_room][to_door] = from_room

    for plan in plans:
        observations = []
        current_room = map_data.startingRoom
        observations.append(map_data.rooms[current_room])

        # 各ドアを通過
        for door_char in plan:
            try:
                door_id = int(door_char)
                if door_id < 0 or door_id > 5:
                    continue

                # 接続があるかチェック
                if current_room in connections and door_id in connections[current_room]:
                    next_room = connections[current_room][door_id]
                    if next_room < len(map_data.rooms):
                        current_room = next_room
                        observations.append(map_data.rooms[current_room])

            except ValueError:
                continue

        results.append(observations)

    return results


@app.post("/register", response_model=RegisterResponse)
async def register(request: RegisterRequest):
    """新しいチームを登録する"""
    team_id = str(uuid.uuid4())

    team = Team(id=team_id, name=request.name, pl=request.pl, email=request.email)

    teams[team_id] = team

    return RegisterResponse(id=team_id)


@app.post("/select", response_model=SelectResponse)
async def select(request: SelectRequest):
    """問題を選択し、ランダムな地図を生成する"""
    if request.problemName not in PROBLEM_SIZES:
        raise HTTPException(status_code=400, detail="Unknown problem name")

    # 新しいチームとみなす
    team = Team(id=request.id, name="negainoido", pl="Rust", email="mail@mail")
    teams[request.id] = team

    # 新しい問題を生成
    problem = generate_random_problem(request.problemName)
    team.current_problem = problem
    team.query_count = 0  # クエリカウントをリセット

    return SelectResponse(problemName=request.problemName)


@app.post("/explore", response_model=ExploreResponse)
async def explore(request: ExploreRequest):
    """エディフィキウムを探検する"""
    if request.id not in teams:
        raise HTTPException(status_code=404, detail="Team not found")

    team = teams[request.id]

    if team.current_problem is None:
        raise HTTPException(status_code=400, detail="No problem selected")

    # ルートプランを実行
    results = simulate_exploration(team.current_problem, request.plans)

    # クエリカウントを更新（プラン数 + リクエストペナルティ1）
    team.query_count += len(request.plans) + 1

    return ExploreResponse(results=results, queryCount=team.query_count)


@app.post("/guess", response_model=GuessResponse)
async def guess(request: GuessRequest):
    """地図を提出する"""
    if request.id not in teams:
        raise HTTPException(status_code=404, detail="Team not found")

    team = teams[request.id]

    if team.current_problem is None:
        raise HTTPException(status_code=400, detail="No problem selected")

    # 地図の正確性をチェック
    is_correct = maps_are_equivalent(team.current_problem, request.map)

    # 本来は不正解であっても問題を選択解除
    # team.current_problem = None

    return GuessResponse(correct=is_correct)


@app.get("/")
async def root():
    """ルートエンドポイント"""
    return {
        "message": "ICFPコンテスト2025 エディフィキウム図書館マッピング API",
        "version": "1.0.0",
        "endpoints": ["/register", "/select", "/explore", "/guess"],
    }


@app.get("/health")
async def health():
    """ヘルスチェックエンドポイント"""
    return {"status": "healthy"}


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8000)
