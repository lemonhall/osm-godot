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
OpenStreetMap 数据 ──→ 建筑轮廓、道路、树木、水域、名称/地址/道路等级
        +
卫星高程数据 ──────→ 真实地形高度
        +
ESA 地表分类 ──────→ 草地/森林/水域/建筑区
        │
        ▼
Godot 项目输出 ──→ project.godot + .tscn 场景 + .tres 材质 + mesh_data JSON + OSM 元数据 + 脚本
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

### 西安雁塔 10 倍命令示例

下面这个命令会生成一个比早期小样例约大 10 倍面积的西安雁塔区域工程。Windows/PowerShell 下如果需要代理访问 Overpass，先设置代理环境变量。

```powershell
$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "34.210594,108.947432,34.226406,108.969568" --output-dir E:\tmp\osm-godot-xian-yanta-style --chunk-size 128
```

### 上海外滩 streaming 示例

这份外滩/陆家嘴区域工程使用 Arnis-style 建筑语法生成，已经用 Godot 4.6 headless 端到端跑过资源导入、运行时 chunk streaming、道路 metadata、OSM 元数据注入、玩家移动和局部加载验证。

```powershell
$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "31.2290,121.4820,31.2455,121.5100" --output-dir E:\tmp\osm-godot-shanghai-bund-v6-streaming --chunk-size 128 --stream-radius 2
```

生成结果：

- 输出工程：`E:\tmp\osm-godot-shanghai-bund-v6-streaming\project.godot`
- 范围：`31.2290,121.4820,31.2455,121.5100`
- 建筑：`1105`
- 道路：`1615`
- 树木：`56`
- 水域：`28`
- 场景元素：`6981`
- 非空区块：`269`

### 上海外滩离线导航示例

这份外滩导航工程在 streaming、player、道路、地面、光照和建筑多样性基础上，额外生成 `navigation_graph.json` 和 `scripts/navigation_controller.gd`。游戏运行时只读取本地 `navigation_index.json` 与 `navigation_graph.json`，不调用高德、Google、OSM、Overpass 或任何外部寻址/路径规划服务。

```powershell
$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "31.2290,121.4820,31.2455,121.5100" --output-dir E:\tmp\osm-godot-shanghai-bund-v7-navigation --chunk-size 128 --stream-radius 2
```

生成结果：

- 输出工程：`E:\tmp\osm-godot-shanghai-bund-v7-navigation\project.godot`
- 路网图：`7711` 个节点、`29986` 条边
- 导航 E2E：从出生点搜索“外滩/外滩源”，生成 `74` 个 waypoint，路线距离约 `1148m`
- Godot 验证：
  ```powershell
  & 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v7-navigation --import --quit
  & 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v7-navigation --script E:\development\osm-godot\tools\godot_navigation_e2e.gd
  ```

### 整个上海 streaming 示例

整上海范围必须使用分片 Overpass 抓取和本地 tile cache。小 chunk 会产生过多文件；本机已验证通过的推荐组合是 `--chunk-size 512 --stream-radius 1`。它仍然生成完整上海 world package，但 Godot 运行时只请求玩家附近最多 `3x3` 个 chunk，并通过加载队列限制同时构建的 chunk 数。

```powershell
$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --release --target-dir E:\tmp\osm-godot-target-release -- --bbox "30.67,120.85,31.88,122.12" --output-dir E:\tmp\osm-godot-shanghai-city-v2-streaming-c512 --chunk-size 512 --stream-radius 1 --tiled-fetch --fetch-tile-degrees 0.25 --tile-cache-dir E:\tmp\osm-godot-cache\shanghai
```

生成结果：

- 输出工程：`E:\tmp\osm-godot-shanghai-city-v2-streaming-c512\project.godot`
- 范围：`30.67,120.85,31.88,122.12`
- 建筑：`192390`
- 道路：`253754`
- 树木：`4315`
- 水域：`6332`
- 场景元素：`1286786`
- 非空区块：`33879`
- 导航索引：`445340` 条道路/建筑记录
- E2E：启动后按队列加载，移动到远处后 chunk 集合变化，且道路 OSM metadata 可读。
- 性能探针：全上海初始请求 `9` 个 chunk，但同时只构建 `2` 个；探针点位显示 `pending_chunks=7`、`loading_chunks=2`、`mesh_instances=2`、`batch_element_total=71`，streamer 稳态刷新约 `4.04us`，不再每帧扫描全量 `33879` 个 chunk。

```powershell
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v2-streaming-c512 --script E:\development\osm-godot\tools\godot_streaming_perf_probe.gd
```

### Arnis-style 建筑语法

生成器会参考 Arnis 的思路，从 OSM 标签推断建筑用途，再用 Godot mesh 组合出更丰富的建筑外观：

- 建筑分类：住宅、办公、酒店、工业、仓库、学校、医院、宗教、历史、高层、温室等。
- 材质与颜色：优先读取 `building:material`、`building:colour`、`roof:material`、`roof:colour`，再回退到类别预设。
- 屋顶语法：支持 `flat`、`gabled`、`hipped`、`skillion`、`pyramidal` 等屋顶；住宅缺省时会偏向坡屋顶。
- 立面细节：按类别生成窗台、百叶、阳台、柱廊、横带、檐口、扶壁、竖向鳍片、玻璃幕墙和屋顶设备。

### OSM 导航元数据

为了后续做路名显示、建筑 POI、驾车 HUD 和路线规划，建筑和道路会把有限 OSM 元数据写入 `mesh_data/*.json`，Godot 加载后挂到节点 meta 上：

- 原始字典：`node.get_meta("osm_metadata")`
- 常用字段：`node.get_meta("osm_id")`、`node.get_meta("osm_kind")`、`node.get_meta("name")`
- 冒号字段会生成安全 key：例如 `addr:housenumber` 可通过 `addr_housenumber` 读取，`building:levels` 可通过 `building_levels` 读取。
- 道路节点会包含 `highway`、`road_width_m`、`name` 等字段；建筑节点会包含 `building`、`amenity`、`shop`、`tourism`、地址、高度或层数等存在于 OSM 的字段。
- `navigation_index.json` 用于本地目的地搜索；`navigation_graph.json` 用于本地 A* 路径规划。Godot 运行时导航不做任何外部 API 请求。

### 参数说明

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--bbox` | 经纬度矩形 (min_lat,min_lng,max_lat,max_lng) | 必填 |
| `--output-dir` | 输出目录 | `./osm_godot_output` |
| `--terrain` | 启用真实高程数据 | 关闭（平坦地形） |
| `--land-cover` | 启用 ESA 地表分类 | 开启 |
| `--scale` | arnis block/meter 比例 | 1.0 |
| `--godot-scale` | Godot 单位/arnis block（1 block = 0.5m） | 0.5 |
| `--chunk-size` | 区块大小（arnis block 单位） | 128 |
| `--stream-radius` | 运行时加载 player 周围的 chunk 半径 | 2 |
| `--tiled-fetch` | 启用大 bbox 分片 Overpass 抓取 | 关闭 |
| `--fetch-tile-degrees` | 分片抓取的经纬度 tile 大小 | 0.05 |
| `--tile-cache-dir` | 分片 OSM JSON 缓存目录 | 无 |
| `--debug` | 调试模式 | 关闭 |
| `--file` | 使用本地 OSM JSON 文件（代替 API 获取） | — |

## 输出结构

```
output/
├── project.godot              # Godot 4.6 项目文件
├── default_environment.tres   # 默认环境
├── metadata.json              # 地理参考元数据
├── world_manifest.json        # streaming chunk manifest
├── navigation_index.json      # 道路/建筑轻量导航索引
├── navigation_graph.json      # 本地可路由道路图
├── assets/                    # 生成资源，例如 cloud_billboard.png
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
│   ├── world_streamer.gd
│   ├── navigation_controller.gd
│   └── fps_player.gd
└── scenes/                    # 场景文件
    ├── master.tscn            # 主场景（天空、太阳、云、玩家、WorldStreamer）
    ├── Chunk_0_0.tscn         # 轻量区块 loader 场景
    ├── Chunk_0_1.tscn
    └── ...
```

`chunk_mesh_loader.gd` 会在线程中读取/解析 chunk JSON，再按每帧预算把同一 chunk 内的 mesh 合批成少量 `MeshInstance3D`，同时为道路/建筑保留轻量 metadata marker。`world_streamer.gd` 通过玩家坐标直接计算 chunk key，同一 chunk 内移动时不会重复扫描 manifest，并用 pending queue 与 `max_concurrent_chunk_loads` 避免跨 chunk 时同步构建多个大块。

## Godot 漫游控制

运行 `scenes/master.tscn` 后会进入 FPS 玩家视角：

- `W/A/S/D` 或方向键：移动
- 鼠标：视角
- `Shift`：加速
- `Space`：跳跃
- `Esc`：释放/捕获鼠标，左键点击可重新捕获
- `V`：noclip 调试穿行模式，用于快速巡检大地图或绕过碰撞卡点
- `N`：打开/关闭离线导航面板，搜索本地 index 中的道路或建筑目的地并开始导航

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
