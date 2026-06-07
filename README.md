# osm-godot

**从真实世界地理数据生成 Godot Engine 3D 城市场景**

osm-godot 是一个 Rust 命令行工具，将 [OpenStreetMap](https://www.openstreetmap.org/) 的矢量数据和卫星高程数据转换为 Godot Engine 4.6 3D 场景工程。选择地球上任意一个区域，自动生成包含建筑、道路、地面、树木、水域、天空光照和 FPS 漫游玩家的 Godot 项目。

本项目灵感来源于 [Arnis](https://github.com/louis-e/arnis)（一个将 OSM 数据转换为 Minecraft 世界的优秀开源项目），复用了其输入管线，将输出替换为 Godot 场景。

> ⚠️ **v1 早期版本** — 功能可用但尚未完全打磨。欢迎贡献！

## 工作原理

```
你选择一个经纬度矩形区域
        │
        ▼
OpenStreetMap 数据 ──→ 建筑轮廓、道路、树木、水域
        +
卫星高程数据 ──────→ 真实地形高度
        +
ESA 地表分类 ──────→ 草地/森林/水域/建筑区
        │
        ▼
Godot 项目输出 ──→ project.godot + .tscn 场景 + .tres 材质 + mesh_data JSON + 脚本
```

## 快速开始

### 安装

```bash
# 克隆仓库
git clone https://github.com/lemon/osm-godot.git
cd osm-godot

# 编译（需要 Rust 工具链）
cargo build --release
```

### 基本用法

```bash
# 生成一个小区域的 Godot 场景（平坦地形，无需下载高程数据）
cargo run --release -- \
  --bbox="48.1360,11.5770,48.1363,11.5775" \
  --output-dir="./my_world"

# 带真实地形 + 地表分类
cargo run --release -- \
  --bbox="37.770,-122.450,37.775,-122.445" \
  --terrain \
  --output-dir="./san_francisco"

# 在 Godot 4.6 编辑器中打开
godot --path "./my_world"
```

### 西安雁塔 10 倍示例

下面这个命令会生成一个比早期小样例约大 10 倍面积的西安雁塔区域工程。Windows/PowerShell 下如果需要代理访问 Overpass，先设置代理环境变量。

```powershell
$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "34.210594,108.947432,34.226406,108.969568" --output-dir E:\tmp\osm-godot-xian-yanta-style-v7-collisionfix --chunk-size 128
```

最近一次生成结果：

- 输出工程：`E:\tmp\osm-godot-xian-yanta-style-v7-collisionfix\project.godot`
- 范围：`34.210594,108.947432,34.226406,108.969568`
- 建筑：`2052`
- 道路：`2051`
- 场景元素：`10655`
- 非空区块：`224`

### 参数说明

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--bbox` | 经纬度矩形 (min_lat,min_lng,max_lat,max_lng) | 必填 |
| `--output-dir` | 输出目录 | `./osm_godot_output` |
| `--terrain` | 启用真实高程数据 | 关闭（平坦地形） |
| `--land-cover` | 启用 ESA 地表分类 | 开启 |
| `--scale` | arnis block/meter 比例 | 1.0 |
| `--godot-scale` | Godot 单位/arnis block（1 block = 0.5m） | 0.5 |
| `--chunk-size` | 区块大小（Godot 单位） | 128 |
| `--debug` | 调试模式 | 关闭 |
| `--file` | 使用本地 OSM JSON 文件（代替 API 获取） | — |

## 输出结构

```
output/
├── project.godot              # Godot 4.6 项目文件
├── default_environment.tres   # 默认环境
├── metadata.json              # 地理参考元数据
├── assets/                    # 生成资源，例如 cloud_billboard.svg
├── materials/                 # toon-ish 材质资源
│   ├── building_wall.tres
│   ├── building_roof.tres
│   ├── road_asphalt.tres
│   ├── terrain_grass.tres
│   ├── water.tres
│   └── ...
├── mesh_data/                 # 每个区块的 ArrayMesh JSON 数据
│   ├── Chunk_0_0.json
│   └── ...
├── scripts/                   # Godot 运行时脚本
│   ├── chunk_mesh_loader.gd
│   └── fps_player.gd
└── scenes/                    # 场景文件
    ├── master.tscn            # 主场景（天空、太阳、云、玩家、所有区块）
    ├── Chunk_0_0.tscn         # 轻量区块 loader 场景
    ├── Chunk_0_1.tscn
    └── ...
```

## Godot 漫游控制

运行 `scenes/master.tscn` 后会进入 FPS 玩家视角：

- `W/A/S/D` 或方向键：移动
- 鼠标：视角
- `Shift`：加速
- `Space`：跳跃
- `Esc`：释放/捕获鼠标，左键点击可重新捕获
- `V`：noclip 调试穿行模式，用于快速巡检大地图或绕过碰撞卡点

玩家会优先出生在道路 mesh 上；如果输入映射在某些 Godot 环境中未被正确解析，脚本也会直接读取键盘按键作为 fallback。

## 技术栈

- **语言**：Rust (edition 2021)
- **引擎**：Godot 4.6 (Forward Plus renderer)
- **数据源**：
  - [OpenStreetMap](https://www.openstreetmap.org/) — 建筑、道路、植被
  - [AWS Terrain Tiles](https://registry.opendata.aws/terrain-tiles/) — 全球高程
  - [USGS 3DEP](https://www.usgs.gov/3d-elevation-program) — 美国高分辨率高程
  - [IGN France](https://geoservices.ign.fr/) — 法国高分辨率高程
  - [ESA WorldCover](https://esa-worldcover.org/) — 全球地表分类

## 参考与致谢

本项目深受 [Arnis](https://github.com/louis-e/arnis) 启发，复用了其优秀的地理数据处理管线。

## 许可证

Apache License 2.0 — 详见 [LICENSE](LICENSE) 文件。
