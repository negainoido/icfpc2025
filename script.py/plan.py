from dataclasses import dataclass


@dataclass
class Goto:
    door: int

    def __str__(self) -> str:
        return f"Goto({self.door})"


@dataclass
class Paint:
    color: int

    def __str__(self) -> str:
        return f"Paint({self.color})"


type Action = Goto | Paint


class Plan:
    def __init__(self, actions: list[Action]):
        self.actions = actions

    @classmethod
    def from_string(cls, plan_str: str) -> "Plan":
        actions = []
        i = 0

        while i < len(plan_str):
            char = plan_str[i]

            if char == "[":
                end_bracket = plan_str.find("]", i)
                if end_bracket == -1:
                    raise ValueError(f"閉じ括弧が見つかりません: 位置 {i}")

                color_str = plan_str[i + 1 : end_bracket]
                if not color_str.isdigit():
                    raise ValueError(f"括弧内は数字である必要があります: '{color_str}'")

                color = int(color_str)
                if color < 0 or color > 5:
                    raise ValueError(f"色番号は0-5の範囲である必要があります: {color}")
                actions.append(Paint(color))
                i = end_bracket + 1

            elif char.isdigit():
                door = int(char)
                if door < 0 or door > 5:
                    raise ValueError(f"ドア番号は0-5の範囲である必要があります: {door}")
                actions.append(Goto(door))
                i += 1

            else:
                raise ValueError(f"無効な文字: '{char}' at position {i}")

        return cls(actions)

    def __str__(self) -> str:
        return f"Plan({self.actions})"

    def __getitem__(self, index) -> Action:
        return self.actions[index]

    def __iter__(self):
        return iter(self.actions)
