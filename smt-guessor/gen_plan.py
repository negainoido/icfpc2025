import random

length = 12  # 生成したい文字列の長さ
result = []
for _ in range(10):
    random_str = "".join(str(random.randint(0, 5)) for _ in range(length))
    result.append(random_str)
print(" ".join(result))

# 32245152403130350510 55031143055055123415 23032532025540534212 24033153512132312203 11245335040305112102
