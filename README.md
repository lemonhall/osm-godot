# osm-godot

**从真实世界地理数据生成 Godot Engine 4.6 流式 3D 城市场景**

osm-godot 是一个 Rust 命令行工具，将 [OpenStreetMap](https://www.openstreetmap.org/) 矢量数据、可选卫星高程数据和 ESA 地表分类转换为可直接打开的 Godot 4.6 项目。生成结果包含分块 mesh 数据、运行时 chunk streaming、Arnis-style 建筑外观、本地导航索引/路网、FPS 漫游玩家、天空光照、离线路线指引和建筑信息查看。

本项目灵感来源于 [Arnis](https://github.com/louis-e/arnis)：复用 OSM/高程/地表分类输入思路，但输出目标是 Godot `project.godot`、`.tscn`、`.tres`、GDScript 和 JSON mesh 数据。

> 当前状态：v1-v4 能力已落地。仓库测试记录显示 `cargo test --target-dir E:\tmp\osm-godot-target` 最近为 91 passed / 1 ignored；大范围上海项目、外滩 streaming、离线导航和 F 键建筑查看均有 Godot 4.6 headless E2E 验证记录。

## 工作原理

```text
经纬度 bbox / 本地 OSM JSON
        |
        v
OpenStreetMap 数据 -> 建筑、道路、铁路、树木、水域、名称、地址、类别标签
        +
可选卫星高程数据 -> 地形高度
        +
ESA 地表分类 -> 草地/建成区/水域等地表材质
        |
        v
Rust 处理管线 -> 分块 mesh JSON + Godot 场景/材质/脚本 + 本地导航数据
        |
        v
Godot 运行时 -> 只加载玩家附近 chunk，支持 FPS 漫游、离线导航、建筑查看
```

## 快速开始

### 安装与构建

```powershell
git clone https://github.com/lemon/osm-godot.git
cd osm-godot
cargo build --release --target-dir E:\tmp\osm-godot-target-release
```

日常开发建议把 Cargo target 放到 `E:\tmp\osm-godot-target`，避免在仓库内产生大量构建产物：

```powershell
cargo check --target-dir E:\tmp\osm-godot-target
cargo test --target-dir E:\tmp\osm-godot-target
```

### 生成一个小区域

```powershell
cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "48.1360,11.5770,48.1363,11.5775" --output-dir E:\tmp\osm-godot-munich-small
```

如果需要通过本机代理访问 Overpass：

```powershell
$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "34.210594,108.947432,34.226406,108.969568" --output-dir E:\tmp\osm-godot-xian-yanta-style --chunk-size 128
```

使用本地 OSM JSON 可以跳过 Overpass 请求：

```powershell
cargo run --target-dir E:\tmp\osm-godot-target -- --file E:\tmp\osm-godot-xian-yanta-style-osm.json --bbox "34.2160,108.9550,34.2210,108.9620" --output-dir E:\tmp\osm-godot-xian-yanta-style-offline --chunk-size 128
```

在 Godot 4.6 中打开：

```powershell
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64.exe' --path E:\tmp\osm-godot-xian-yanta-style
```

## 常用示例

### 上海外滩 streaming + 离线导航

生成外滩/陆家嘴局部工程。运行时会读取 `world_manifest.json`，只加载玩家附近 chunk；导航只读取本地 `navigation_index.json` 和 `navigation_graph.json`，不调用高德、Google、OSM、Overpass 或任何外部路径规划服务。

```powershell
$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "31.2290,121.4820,31.2455,121.5100" --output-dir E:\tmp\osm-godot-shanghai-bund-navigation --chunk-size 128 --stream-radius 2
```

历史验证记录中的外滩导航样例：

- 输出工程：`E:\tmp\osm-godot-shanghai-bund-v7-navigation\project.godot`
- 路网图：`7711` 个节点、`29986` 条边
- 导航 E2E：从出生点搜索“外滩/外滩源”，生成 `74` 个 waypoint，路线距离约 `1148m`
- UI 行为：按 `N` 打开居中导航面板，释放鼠标并暂停 FPS 控制；按“取消”恢复玩家控制
- 路线指引：亮绿色连续路线带 + 终点到达圈；到达后自动清理路线、到达圈和 HUD

Godot headless 验证命令：

```powershell
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v7-navigation --import --quit
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v7-navigation --script E:\development\osm-godot\tools\godot_navigation_e2e.gd
```

### 整个上海 world package

整上海范围必须使用分片 Overpass 抓取和本地 tile cache。推荐组合是 `--chunk-size 512 --stream-radius 1`：它仍生成完整 world package，但 Godot 运行时只请求玩家附近最多 `3x3` 个 chunk，并限制同时构建的 chunk 数。

```powershell
$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --release --target-dir E:\tmp\osm-godot-target-release -- --bbox "30.67,120.85,31.88,122.12" --output-dir E:\tmp\osm-godot-shanghai-city-navigation-c512 --chunk-size 512 --stream-radius 1 --tiled-fetch --fetch-tile-degrees 0.25 --tile-cache-dir E:\tmp\osm-godot-cache\shanghai
```

历史验证记录中的全上海 streaming 样例：

- 输出工程：`E:\tmp\osm-godot-shanghai-city-v2-streaming-c512\project.godot`
- 建筑：`192390`
- 道路：`253754`
- 树木：`4315`
- 水域：`6332`
- 场景元素：`1286786`
- 非空区块：`33879`
- 导航索引：`445340` 条道路/建筑记录
- 性能探针：初始请求 `9` 个 chunk，同时构建 `2` 个；streamer 稳态刷新约 `4.04us`

性能探针：

```powershell
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v2-streaming-c512 --script E:\development\osm-godot\tools\godot_streaming_perf_probe.gd
```

## 已实现能力

- **Arnis-style 建筑**：按 OSM 标签推断住宅、办公、酒店、工业、仓库、学校、医院、宗教、历史、高层、温室等建筑类别。
- **材质与颜色**：优先读取 `building:material`、`building:colour`、`roof:material`、`roof:colour`，再回退到类别预设材质。
- **屋顶与立面**：支持 `flat`、`gabled`、`hipped`、`skillion`、`pyramidal` 等屋顶；生成窗、门、阳台、横带、檐口、扶壁、玻璃幕墙、屋顶设备等细节。
- **道路/铁路/水域/树木**：道路按宽度生成带状 mesh，铁路和水域随 chunk 输出，树木来自 `natural=tree` 节点。
- **运行时 streaming**：`world_streamer.gd` 根据玩家位置加载/卸载 chunk，不再让 `master.tscn` 静态引用所有 chunk。
- **分块 JSON mesh**：chunk `.tscn` 是轻量 loader，几何在 `mesh_data/Chunk_X_Z.json`，运行时用 `ArrayMesh` 批量构建。
- **本地导航**：生成 `navigation_index.json` 和 `navigation_graph.json`；Godot 运行时用本地 A* 搜索路线。
- **建筑信息查看**：按 `F` 查看眼前已加载建筑的本地 OSM 名称和关键属性，信息卡约 5 秒后隐藏。
- **中文门匾**：带正式中文名的建筑会在 chunk 加载时生成白底黑字门匾；普通建筑为门旁竖牌，店铺/服务类为门上横匾；英文名或无名建筑不挂牌。

## OSM 元数据

建筑和道路会把有限 OSM 元数据写入 `mesh_data/*.json`，Godot chunk loader 加载后挂到 metadata marker 上：

- 原始字典：`node.get_meta("osm_metadata")`
- 常用字段：`node.get_meta("osm_id")`、`node.get_meta("osm_kind")`、`node.get_meta("name")`
- 中文名称字段：`name:zh`、`official_name:zh`、`alt_name:zh`、`brand:zh`、`operator:zh`，也兼容 `zh-Hans` / `zh-Hant` 变体
- 冒号字段会生成安全 key：例如 `addr:housenumber` 可通过 `addr_housenumber` 读取，`building:levels` 可通过 `building_levels` 读取
- `navigation_index.json` 用于本地目的地搜索；`navigation_graph.json` 用于本地 A* 路径规划

## 参数说明

| 参数 | 说明 | 默认值 |
|---|---|---|
| `--bbox` | 经纬度矩形：`min_lat,min_lng,max_lat,max_lng` | 必填 |
| `--file` | 使用本地 OSM JSON 文件，代替 Overpass 请求 | 无 |
| `--save-json-file` | 保存下载到的 OSM JSON，便于复用 | 无 |
| `--output-dir` / `--path` | 输出 Godot 项目目录 | `./osm_godot_output` |
| `--scale` | arnis blocks per meter | `1.0` |
| `--godot-scale` | Godot 米/arnis block；`0.5` 表示 1 block = 0.5m | `0.5` |
| `--ground-level` | 平坦地形的 arnis block 高度 | `0` |
| `--terrain` | 启用真实 DEM 高程数据 | 关闭 |
| `--land-cover` | 启用 ESA WorldCover 地表分类 | 开启 |
| `--chunk-size` | chunk 大小，单位是 arnis block；Godot 尺寸为 `chunk-size * godot-scale` | `128` |
| `--stream-radius` | 运行时加载玩家周围的 chunk 半径 | `2` |
| `--tiled-fetch` | 大 bbox 分片 Overpass 抓取 | 关闭 |
| `--fetch-tile-degrees` | 分片抓取 tile 经纬度大小 | `0.04` |
| `--tile-cache-dir` | 分片 OSM JSON 缓存目录 | 无 |
| `--downloader` | 下载器方法：`requests` / `curl` / `wget` | `requests` |
| `--debug` | 输出额外调试信息 | 关闭 |

## 输出结构

```text
output/
├── project.godot              # Godot 4.6 项目文件，主场景为 res://scenes/master.tscn
├── default_environment.tres   # 默认环境资源
├── metadata.json              # scale/chunk/坐标系元数据；当前地理边界仍为占位值
├── world_manifest.json        # streaming chunk manifest
├── navigation_index.json      # 道路/建筑轻量导航索引
├── navigation_graph.json      # 本地可路由道路图
├── assets/
│   └── cloud_billboard.png
├── materials/
│   ├── building_wall.tres
│   ├── road_asphalt.tres
│   ├── terrain_grass.tres
│   └── ...
├── mesh_data/
│   ├── Chunk_0_0.json
│   └── ...
├── scripts/
│   ├── chunk_mesh_loader.gd
│   ├── world_streamer.gd
│   ├── navigation_controller.gd
│   └── fps_player.gd
└── scenes/
    ├── master.tscn
    ├── Chunk_0_0.tscn
    └── ...
```

`chunk_mesh_loader.gd` 在线程中读取/解析 chunk JSON，再按每帧预算把同一 chunk 内的 mesh 合批成少量 `MeshInstance3D`，同时保留道路/建筑 metadata marker。`world_streamer.gd` 通过玩家坐标直接计算 chunk key，用 pending queue 和 `max_concurrent_chunk_loads` 避免跨 chunk 时同步构建多个大块。

建筑牌匾也由 `chunk_mesh_loader.gd` 在 chunk 加载时生成，不写入 `master.tscn`。它只使用本地 metadata：优先 `official_name:zh` / `name:zh`，其次是纯中文 `official_name` / `name`，不会翻译英文名，也不会从 `building` / `amenity` 类别合成假中文名。

## Godot 漫游控制

运行 `scenes/master.tscn` 后会进入 FPS 玩家视角：

- `W/A/S/D` 或方向键：移动
- 鼠标：视角
- `Shift`：加速
- `Space`：跳跃
- `Esc`：释放/捕获鼠标，左键点击可重新捕获
- `V`：noclip 调试穿行模式
- `N`：打开/关闭离线导航面板；面板打开时释放鼠标并暂停 FPS 控制
- `G`：导航路线存在时开启/关闭自动巡航；手动移动会退出自动巡航
- `F`：查看眼前已加载建筑的本地 OSM 名称和关键属性

玩家会优先出生在道路 mesh 上；如果输入映射在某些 Godot 环境中未被正确解析，脚本也会直接读取键盘按键作为 fallback。

## 技术栈

- **语言**：Rust 2021
- **引擎**：Godot 4.6，Forward Plus renderer
- **主要 Rust 依赖**：`clap`、`reqwest`、`serde_json`、`geo`、`image`、`rayon`、`parquet`
- **数据源**：
  - [OpenStreetMap](https://www.openstreetmap.org/)：建筑、道路、植被、水域等矢量数据
  - [AWS Terrain Tiles](https://registry.opendata.aws/terrain-tiles/)：全球高程
  - [USGS 3DEP](https://www.usgs.gov/3d-elevation-program)：美国高分辨率高程
  - [IGN France](https://geoservices.ign.fr/) / IGN Spain：欧洲区域高程
  - [ESA WorldCover](https://esa-worldcover.org/)：全球地表分类

## 开发与验证

```powershell
cargo check --target-dir E:\tmp\osm-godot-target
cargo test --target-dir E:\tmp\osm-godot-target
cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
git diff --check
```

Godot 不一定在 PATH 中，验证前可先查找：

```powershell
where.exe godot
where.exe godot4
```

已提供的 Godot E2E 脚本位于 `tools/`：`godot_player_e2e.gd`、`godot_streaming_e2e.gd`、`godot_streaming_perf_probe.gd`、`godot_navigation_e2e.gd`、`godot_navigation_autorun_e2e.gd`、`godot_building_inspection_e2e.gd`。

## 参考与致谢

本项目深受 [Arnis](https://github.com/louis-e/arnis) 启发，复用了其优秀的地理数据处理思路。

## 许可证

`Cargo.toml` 当前声明为 Apache-2.0。
