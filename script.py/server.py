#!/usr/bin/env python3
"""
ICFPコンテスト2025 エディフィキウム図書館マッピング モックサーバー
FastAPIを使用してすべてのプロトコルを実装
"""

import copy
import json
import logging
import random
import uuid
from dataclasses import dataclass
from pathlib import Path

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

from plan import Goto, Paint, Plan

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
    plans: list[str]


class ExploreResponse(BaseModel):
    results: list[list[int]]
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
    rooms: list[int]
    startingRoom: int
    connections: list[Connection]


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
DOOR_LIMITS = {
    "__default": 18,
    "aleph": 6,
    "beth": 6,
    "gimel": 6,
    "daleth": 6,
    "he": 6,
    "vau": 6,
    "zain": 6,
    "hhet": 6,
    "teth": 6,
    "iod": 6,
}


@dataclass
class Room:
    """部屋を表すクラス"""

    label: int  # 2ビット整数 (0-3)
    doors: dict[int, tuple]  # door_id -> (connected_room_id, connected_door_id)


@dataclass
class Problem:
    """問題インスタンスを表すクラス"""

    name: str
    rooms: list[Room]
    limit: int  # Limit of door-steps
    starting_room: int = 0

    @classmethod
    def from_map_data(cls, map_data: "MapData") -> "Problem":
        """MapDataからProblemを作成する"""
        # 部屋を作成
        rooms = []
        for label in map_data.rooms:
            room = Room(label=label, doors={})
            rooms.append(room)

        # 接続情報を設定（無向グラフなので双方向に設定）
        for conn in map_data.connections:
            from_room = conn.from_.room
            from_door = conn.from_.door
            to_room = conn.to.room
            to_door = conn.to.door

            # from -> to の接続
            rooms[from_room].doors[from_door] = (to_room, to_door)
            # to -> from の接続（無向グラフ）
            rooms[to_room].doors[to_door] = (from_room, from_door)

        return cls(
            name="submitted_map",
            rooms=rooms,
            starting_room=map_data.startingRoom,
            limit=DOOR_LIMITS["__default"],
        )


@dataclass
class Team:
    """チーム情報を表すクラス"""

    id: str
    name: str
    pl: str
    email: str
    current_problem: Problem | None = None
    query_count: int = 0


# グローバル状態
teams: dict[str, Team] = {}


def build_problem(problem_name: str, size: int, limit: int) -> Problem:
    problem = get_solved_problem(problem_name)
    if problem is None:
        logger.info(
            f"No known problem found for {problem_name}, generating random problem"
        )
        problem = generate_random_problem(problem_name, size, limit)

    # 生成された問題情報をログ出力
    print(f"\n=== 生成された問題 ({problem_name}) ===")
    print(f"部屋数: {size}")
    print(f"ドア制限: x{limit}")
    print(f"開始部屋: {problem.starting_room}")
    print("部屋ラベル:")
    for i, room in enumerate(problem.rooms):
        print(f"  部屋{i}: ラベル{room.label}")
    print("\nJSON形式グラフ:")
    json_graph = generate_json_graph(problem)
    print(json_graph)
    print("=" * 50)

    # JSON形式の地図を /tmp/map.json に保存
    with open("/tmp/map.true.json", "w", encoding="utf-8") as f:
        print("Saved as /tmp/map.true.json")
        f.write(json_graph)

    return problem


def get_solved_problem(problem_name: str) -> Problem | None:
    """解決済み既知の問題を取得する

    Parameters
    ----------
    problem_name
        aleph 以上を想定

    Returns
    -------
        ./problems/{problem_name}/*.json があれば, ここからランダムに一つ取得する
        無ければ None を返す
    """
    server_dir = Path(__file__).parent
    problems_dir = server_dir / "problems" / problem_name
    if not problems_dir.exists():
        return None

    json_files = list(problems_dir.glob("*.json"))
    if not json_files:
        return None

    selected_file = random.choice(json_files)

    try:
        with open(selected_file, "r", encoding="utf-8") as f:
            data = json.load(f)
        if "api_history" not in data or not data["api_history"]:
            logger.warning(f"No api_history in {selected_file}")
            return None

        last_request = data["api_history"][-1]
        if "request" not in last_request or "map" not in last_request["request"]:
            logger.warning(f"No map in last request of {selected_file}")
            return None

        map_data_dict = last_request["request"]["map"]

        map_data = MapData(**map_data_dict)
        problem = Problem.from_map_data(map_data)
        problem.name = problem_name

        limit = DOOR_LIMITS.get(problem_name, DOOR_LIMITS["__default"])
        problem.limit = limit

        logger.info(f"Loaded problem from {selected_file}")
        return problem

    except Exception as e:
        logger.warning(f"Failed to load problem from {selected_file}: {e}")
        return None


def generate_random_problem(problem_name: str, size: int, limit: int) -> Problem:
    """指定された問題名に基づいてランダムな問題を生成する

    Parameters
    ----------
    problem_name
        任意の問題名（例: "probatio", "primus", "aleph", "beth", など）
    size
        部屋の数
    limit
        ドアステップの制限 (6, 18)
    """

    # 各部屋にできるだけ均等な2ビットラベルを割り当て
    offset = random.randint(0, 3)
    labels = [(i + offset) % 4 for i in range(size)]
    random.shuffle(labels)

    rooms = []
    for label in labels:
        room = Room(label=label, doors={})
        rooms.append(room)

    # 無向グラフになるように接続を生成
    # 未接続の(room, door)ペアを管理
    unconnected_vertices = set()
    for room_id in range(size):
        for door_id in range(6):  # ドア0-5
            unconnected_vertices.add((room_id, door_id))

    # 未接続のペアから2つ選んで接続
    while len(unconnected_vertices) > 0:
        vertex1 = random.choice(list(unconnected_vertices))
        vertex2 = random.choice(list(unconnected_vertices))
        unconnected_vertices.remove(vertex1)
        if vertex1 != vertex2:
            unconnected_vertices.remove(vertex2)

        room1, door1 = vertex1
        room2, door2 = vertex2

        # 無向エッジを作成（双方向に接続）
        rooms[room1].doors[door1] = (room2, door2)
        rooms[room2].doors[door2] = (room1, door1)

    problem = Problem(name=problem_name, rooms=rooms, starting_room=0, limit=limit)

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


def simulate_exploration(problem: Problem, plans: list[Plan]) -> list[list[int]]:
    """ルートプランを実行して観察結果を返す"""
    results = []

    for plan in plans:
        observations = []
        current_room = problem.starting_room
        problem_copy = copy.deepcopy(problem)

        # 開始部屋のラベルを記録
        observations.append(problem_copy.rooms[current_room].label)

        # 各アクションを実行
        for action in plan.actions:
            if isinstance(action, Goto):
                door_id = action.door
                # 現在の部屋の指定されたドアを通過
                if door_id in problem_copy.rooms[current_room].doors:
                    next_room, _ = problem_copy.rooms[current_room].doors[door_id]
                    current_room = next_room
                    observations.append(problem_copy.rooms[current_room].label)
                else:
                    # ドアが存在しない場合（通常はすべてのドアが存在するはず）
                    observations.append(problem_copy.rooms[current_room].label)
            elif isinstance(action, Paint):
                # 現在の部屋のラベルを指定した色に変更
                problem_copy.rooms[current_room].label = action.color
                observations.append(problem_copy.rooms[current_room].label)

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
    n = len(problem.rooms)
    test_plans = []

    # 長さ (length_multiplier * n) のそれぞれでランダム生成
    for length_multiplier in [5, 10, 18]:
        plan_length = n * length_multiplier
        for _ in range(20):
            # ランダムなドア番号（0-5）を生成
            plan = "".join(str(random.randint(0, 5)) for _ in range(plan_length))
            test_plans.append(plan)

    for plan_str in test_plans:
        plan = Plan.from_string(plan_str)
        original_results = simulate_exploration(problem, [plan])
        submitted_results = simulate_submitted_map(submitted_map, [plan_str])

        if original_results != submitted_results:
            logger.warning(f"プラン '{plan}' で結果が一致しない")
            logger.warning(f"期待される結果: {original_results}")
            logger.warning(f"提出された結果: {submitted_results}")
            return False

    return True


def simulate_submitted_map(map_data: MapData, plan_strs: list[str]) -> list[list[int]]:
    """提出された地図でルートプランをシミュレート"""
    problem = Problem.from_map_data(map_data)
    plans = [Plan.from_string(plan_str) for plan_str in plan_strs]
    return simulate_exploration(problem, plans)


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
    if request.problemName in PROBLEM_SIZES:
        # 既知の問題から選ぶ場合
        problem_name = request.problemName
        size = PROBLEM_SIZES[request.problemName]
        limit = DOOR_LIMITS.get(request.problemName) or DOOR_LIMITS["__default"]
    elif request.problemName.isdigit():
        # 任意の部屋数を指定する場合
        size = int(request.problemName)
        limit = DOOR_LIMITS["__default"]
        problem_name = f"custom:{size}:{limit}"
    elif ":" in request.problemName:
        # カスタム問題名 (例: "10:6" なら size=10, limit=6)
        req_parts = request.problemName.split(":")
        size = int(req_parts[-2].strip())
        limit = int(req_parts[-1].strip())
        problem_name = f"{size}:{limit}"
    else:
        raise HTTPException(status_code=400, detail="Unknown problem name format")

    # 常に新しいチームとみなす
    team = Team(id=request.id, name="negainoido", pl="Rust", email="mail@mail")
    teams[request.id] = team

    # 新しい問題を生成
    problem = build_problem(problem_name, size, limit)
    team.current_problem = problem
    team.query_count = 0  # クエリカウントをリセット

    return SelectResponse(problemName=request.problemName)


@app.post("/explore", response_model=ExploreResponse)
async def explore(request: ExploreRequest):
    """エディフィキウムを探検する"""

    logger.info("Explore request received: %s", request)

    if request.id not in teams:
        raise HTTPException(status_code=404, detail="Team not found")

    team = teams[request.id]
    logger.info("Team found: %s", team)

    if team.current_problem is None:
        raise HTTPException(status_code=400, detail="No problem selected")

    # プランの解析
    try:
        plans = [Plan.from_string(plan_str) for plan_str in request.plans]
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))

    # ドア移動の制限をチェック
    n = len(team.current_problem.rooms)
    max_plan_length = n * team.current_problem.limit
    for plan in plans:
        if plan.num_goto() > max_plan_length:
            raise HTTPException(
                status_code=400,
                detail=f"Plan has too many doors (Goto actions): {plan.num_goto()} exceeds maximum {max_plan_length}",
            )

    # ルートプランを実行
    results = simulate_exploration(team.current_problem, plans)

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
