# PRD-0002: 大范围城市世界包与运行时流式加载

## Vision

用户可以生成整个上海级别的大范围 Godot 城市工程，但运行时不会一次性加载全城。主场景只加载玩家附近有限半径内的 chunk，道路、地面、建筑和 OSM 元数据都随 chunk 进入/离开场景；未来驾车漫游时，玩家能从一个区域自然驶向另一个区域，而内存、场景树节点数和资源加载压力保持在可控范围内。

## Background

v1 已经把建筑、道路、地面、光照、player 和 OSM 元数据打通，也已经按 chunk 写出建筑/地面场景。但当前 `master.tscn` 会把所有非空 chunk 作为 `ExtResource` 写入并实例化，`roads.tscn` 也会加载全图 `roads.json`。这对外滩规模可用，对整个上海不成立。

大范围城市需要把“离线生成完整世界”和“运行时只加载局部视野”分开：

- 离线生成：可以生成很多 chunk、道路 chunk 和索引文件。
- 运行时加载：只根据 player 位置加载附近 chunk，远处 chunk 释放。
- 导航准备：保留全城轻量道路/POI 索引，但不实例化全城 mesh。
- 数据抓取：不能只用一个巨大 Overpass bbox，需要分片请求、缓存和去重。

## Terms

- **World package**：一个可打开的 Godot 项目，包含 `world_manifest.json`、chunk JSON、道路 chunk JSON、导航索引和主场景。
- **Runtime streaming**：Godot 运行时根据玩家位置动态加载/卸载 chunk。
- **Chunk radius**：以玩家所在 chunk 为中心，保留的加载半径。半径 2 表示最多保留 5x5 个 chunk。
- **Road chunk**：按 chunk 切分后的道路 mesh 数据，不再使用全图单个 `roads.json`。
- **Navigation index**：轻量 JSON 索引，保留道路/建筑名称、OSM id、bbox、类型和中心点，供未来搜索、HUD、路线规划使用。
- **Tiled fetch**：按经纬度把大 bbox 分成多个子 bbox 请求 Overpass，并合并去重。

## Requirements

### REQ-0002-001: 主场景不静态引用所有 chunk

- **Motivation**：整个上海的 chunk 数量会远超 Godot 主场景一次性引用/实例化的承载能力。
- **Scope**：生成 `world_manifest.json`，`master.tscn` 只引用 player、环境、streamer 脚本和少量资源，不再为每个 chunk 写 `ExtResource` 或静态 chunk 节点。
- **Non-goals**：不改成网络流媒体；本轮仍是本地 Godot 项目。
- **Acceptance**：单元测试证明生成 20 个以上非空 chunk 时，`master.tscn` 不包含 `Chunk_*.tscn` 的 `ExtResource`，但 `world_manifest.json` 包含这些 chunk 条目。

### REQ-0002-002: Godot 运行时按 player 位置加载/卸载 chunk

- **Motivation**：玩家漫游只需要看到附近区域，远处 mesh 应释放。
- **Scope**：新增 `world_streamer.gd`，读取 manifest，按 player 所在 chunk 和 `stream_radius` 实例化附近 chunk；超出 `unload_radius` 的 chunk queue_free；支持启动时立即加载出生点周围 chunk。
- **Non-goals**：不实现复杂 LOD、遮挡剔除或后台线程优先级调度；可先用同步 load，但接口要允许后续切换到 threaded load。
- **Acceptance**：Godot E2E 验证启动后只加载有限 chunk，模拟移动到远处后 chunk 集合发生变化，且加载数量不超过半径上限。

### REQ-0002-003: 道路按 chunk 输出并参与 streaming

- **Motivation**：全图 `roads.json` 会成为大地图瓶颈，而且道路应和建筑/地面同生命周期。
- **Scope**：道路 mesh 不再写入全图 `roads.json`，而是写入每个 chunk 的道路 JSON 或与 chunk JSON 同包；streamer 加载 chunk 时加载对应道路节点，OSM road metadata 仍可读。
- **Non-goals**：本轮不做跨 chunk 道路拓扑连通图；只保证可见道路随 chunk 加载。
- **Acceptance**：单元测试证明输出目录不依赖全图 `roads.json`，道路元素在 chunk JSON 中保留 `osm_id`、`osm_kind=road`、`name`、`highway`。

### REQ-0002-004: 生成全城轻量导航索引

- **Motivation**：未来导航和驾车需要在不加载全城 mesh 的情况下查询道路名、建筑名和位置。
- **Scope**：输出 `navigation_index.json`，包含道路和建筑的 `osm_id`、`osm_kind`、`name`、类别/等级、chunk 坐标、中心点和 bbox。
- **Non-goals**：不做路径规划算法、转向指令、HUD、车辆物理。
- **Acceptance**：集成测试证明外滩样例索引中至少包含道路和建筑记录，且记录可追溯到 chunk。

### REQ-0002-005: 大 bbox 支持分片 Overpass 抓取和去重

- **Motivation**：整个上海单次 Overpass 请求容易超时或内存溢出。
- **Scope**：新增 CLI 参数控制分片抓取：启用后把 bbox 切成多个子 bbox，逐片请求或读取缓存，合并 OSM element 时按 `(type,id)` 去重，保存 tile cache。
- **Non-goals**：不做分布式下载、不并发压爆 Overpass、不保证所有服务器对超大城市都一次成功。
- **Acceptance**：单元测试覆盖 bbox 切分和 OSM 去重；集成命令能生成上海大范围 world package，并记录 tile cache 和 manifest。

### REQ-0002-006: 上海大范围 world package 可生成且可用 E2E 验证局部加载

- **Motivation**：用户明确目标是生成整个上海，并用游戏人物局部漫游。
- **Scope**：提供 README 命令生成上海大范围项目；Godot 4.6 import 通过；E2E 在大范围项目上验证 `master.tscn` 可运行、player 可移动、streamer 只加载局部 chunk、OSM 元数据可读。
- **Non-goals**：不要求一次 E2E 走完整个上海；不要求视觉上每个真实地标完全一致。
- **Acceptance**：README 记录命令和输出路径；Godot import exit 0；E2E exit 0，日志包含 loaded chunk count、manifest chunk count、stream radius、road/building metadata。

## Global Non-goals

- 不做真实车辆驾驶系统、路线规划、导航 UI、语音播报。
- 不引入数据库或服务端，所有产物仍是本地文件。
- 不把建筑改回全量静态加载。
- 不为了大地图牺牲 v1 的 Godot 4.6、player、光照、道路、地面和 OSM 元数据能力。

## Verification Summary

```powershell
cargo test --target-dir E:\tmp\osm-godot-target world_streaming -- --nocapture
cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
cargo test --target-dir E:\tmp\osm-godot-target
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v6-streaming --import
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v6-streaming --script E:\development\osm-godot\tools\godot_streaming_e2e.gd
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v2-streaming-c512 --import
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v2-streaming-c512 --script E:\development\osm-godot\tools\godot_streaming_e2e.gd
```
