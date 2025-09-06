from matplotlib import font_manager

font_path = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"

prop = font_manager.FontProperties(fname=font_path)
name = prop.get_name()
print(name)
