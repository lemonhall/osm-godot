# v2 Plan: World package streaming 与上海大范围生成

## Goal

把当前“chunk 已离线切分但主场景全量加载”的 Godot 输出改成真正的大世界 streaming：生成完整 world package，但 Godot 运行时只实例化 player 附近 chunk。完成后外滩范围和上海大范围都必须能生成，且大范围项目打开后不会一次性加载全城 mesh。

## PRD Trace

- REQ-0002-001：主场景不静态引用所有 chunk
- REQ-0002-002：Godot 运行时按 player 位置加载/卸载 chunk
- REQ-0002-003：道路按 chunk 输出并参与 streaming
- REQ-0002-004：生成全城轻量导航索引
- REQ-0002-005：大 bbox 支持分片 Overpass 抓取和去重
- REQ-0002-006：上海大范围 world package 可生成且可用 E2E 验证局部加载

## Scope

本轮会修改或新增：

- `src/args.rs`
- `src/main.rs`
- `src/retrieve_data.rs`
- `src/osm_parser.rs`
- `src/scene_writer/chunk_grid.rs`
- `src/scene_writer/mod.rs`
- `src/scene_writer/tscn_writer.rs`
- `src/scene_writer/project_writer.rs`
- `tools/godot_streaming_e2e.gd`
- `README.md`

本轮不做：

- 不做车辆物理、路线规划、导航 HUD、语音导航。
- 不引入数据库、服务端或网络运行时。
- 不做复杂 LOD、遮挡剔除、地平线远景 impostor。
- 不把建筑/道路视觉质量降级回白盒。

## Acceptance

- `master.tscn` 不再为每个 chunk 写 `ExtResource("res://scenes/Chunk_...")`，也不静态实例化所有 chunk。
- 输出 `world_manifest.json`，每个非空 chunk 都有坐标、Godot 原点、bounds、scene path、mesh data path、元素数量。
- 输出 `navigation_index.json`，包含道路和建筑元数据记录。
- 道路不再依赖全图 `mesh_data/roads.json`；道路 mesh 按 chunk 保存在 chunk 数据中，metadata 不丢失。
- `world_streamer.gd` 能按 player chunk 和半径加载/卸载 chunk。
- 新增分片抓取参数，bbox tiling 和 OSM element 去重有单元测试。
- 外滩 streaming 项目和上海大范围 streaming 项目均能 Godot 4.6 headless import。
- `tools\godot_streaming_e2e.gd` 在两个项目上通过，日志包含 manifest chunk count、loaded chunk count、stream radius、OSM metadata。

## Steps

1. 写失败测试（Red）
   - `scene_writer`：断言 `world_manifest.json` 存在且 chunk 条目数等于非空 chunk 数。
   - `scene_writer`：断言 `master.tscn` 不包含 `Chunk_*.tscn` ext_resource，不包含静态 chunk node。
   - `tscn_writer`：断言 road mesh 被保留在 chunk JSON，且 `roads.json` 不再作为必需产物。
   - `scene_writer`：断言 `world_streamer.gd` 包含 manifest 读取、radius、load/unload 函数和 player path。
   - `navigation_index`：断言道路/建筑记录包含 `osm_id`、`osm_kind`、`name`、chunk、center、bbox。
   - `retrieve_data` 或新模块：断言大 bbox 可按 tile 大小切分，合并 OSM JSON 时 `(type,id)` 去重。

2. 跑到红
   - 命令：
     ```powershell
     cargo test --target-dir E:\tmp\osm-godot-target world_streaming -- --nocapture
     cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
     ```
   - 预期：新增测试失败，失败原因对应 manifest、streamer、道路 chunk 化、导航索引或 tiled fetch 缺失。

3. 实现到绿
   - 增加 manifest/navigation index 数据结构和 JSON 写出。
   - 改 `master.tscn`：只保留环境、player、streamer，不静态引用 chunk/roads。
   - 新增 `world_streamer.gd`：根据 player 坐标计算 chunk，维护 loaded set，加载半径内 chunk，卸载远处 chunk。
   - 改 chunk JSON：道路和非道路元素同 chunk 输出；删除全图 roads scene 依赖。
   - 新增 bbox tiling、tile cache、OSM data merge dedupe。
   - CLI 增加分片抓取和 streaming 相关参数。

4. 跑到绿
   - 命令：
     ```powershell
     cargo test --target-dir E:\tmp\osm-godot-target world_streaming -- --nocapture
     cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
     cargo test --target-dir E:\tmp\osm-godot-target
     ```
   - 预期：全部 exit code 为 0。

5. 生成外滩 streaming 项目
   - 命令：
     ```powershell
     $env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "31.2290,121.4820,31.2455,121.5100" --output-dir E:\tmp\osm-godot-shanghai-bund-v5-streaming --chunk-size 128 --stream-radius 2
     ```
   - 预期：生成成功，包含 `world_manifest.json`、`navigation_index.json`、`scripts/world_streamer.gd`。

6. 生成上海大范围 streaming 项目
   - 命令：
     ```powershell
     $env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --release --target-dir E:\tmp\osm-godot-target-release -- --bbox "30.67,120.85,31.88,122.12" --output-dir E:\tmp\osm-godot-shanghai-city-v2-streaming-c512 --chunk-size 512 --stream-radius 1 --tiled-fetch --fetch-tile-degrees 0.25 --tile-cache-dir E:\tmp\osm-godot-cache\shanghai
     ```
   - 预期：生成成功；若 Overpass 网络失败，缓存目录保留已完成 tile，计划回顾记录失败 tile 和复跑命令。
   - 记录：整上海 `--chunk-size 128` 会产生过多 chunk 文件，生成和 Godot import 成本过高；`512 + stream_radius 1` 是本机验收通过的全城包参数。

7. Godot import + E2E
   - 命令：
     ```powershell
     & 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v5-streaming --import
     & 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v5-streaming --script E:\development\osm-godot\tools\godot_streaming_e2e.gd
     & 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v2-streaming-c512 --import
     & 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v2-streaming-c512 --script E:\development\osm-godot\tools\godot_streaming_e2e.gd
     ```
   - 预期：全部 exit code 为 0；E2E 验证局部加载而不是全量加载。

8. 文档回写与回顾
   - 更新 README：记录 streaming 机制、外滩 v5 命令、上海大范围命令、项目路径。
   - 更新 `docs/plan/v2-index.md`：M1-M5 状态和证据。
   - 编码检查：`git diff --text -- README.md docs` + 乱码扫描。

## Risks

- **Overpass 限流/失败**：上海全量数据可能多 tile 失败。缓解：tile cache 幂等保存；失败时可复跑，已完成 tile 不重下。
- **Godot 运行时卡顿**：同步 load chunk 可能造成瞬间卡顿。缓解：先控制半径和 chunk 大小，streamer 接口保留 threaded load 替换点。
- **跨 chunk 道路接缝**：道路按 chunk 切分后可能边界处有短断。缓解：本轮以可见和 metadata 为主，不承诺拓扑级连续。
- **坐标精度**：上海大范围会有较大 Godot 坐标。缓解：chunk 内 mesh 保持局部坐标，后续可加 floating origin；本轮 E2E 验证局部区域可走动。
