# v2: 大范围城市 world package 与运行时 streaming

## Vision

关联 PRD：[PRD-0002: 大范围城市世界包与运行时流式加载](../prd/PRD-0002-world-streaming-shanghai.md)

本轮目标是让“整个上海”这种大 bbox 可以生成成本地 Godot world package，并让 Godot 运行时只加载玩家附近 chunk。外滩 v4 的视觉质量、道路、地面、光照、player、OSM 元数据必须保留。

## Milestones

| Milestone | Scope | DoD | Verification | Status |
|---|---|---|---|---|
| M1 文档与验收冻结 | PRD-0002、v2 index、v2 计划 | 每条 REQ 都有计划、测试或 E2E；DoD 可二元判定 | `git diff --text -- docs` + 编码扫描 | done |
| M2 v2.1 外滩 streaming | manifest + master 不全量引用 + runtime streamer | 外滩项目启动只加载半径内 chunk，移动后 chunk 集合变化 | `cargo test ... scene_writer` + Godot 外滩 E2E | done |
| M3 v2.2 道路 chunk 化 | 道路随 chunk 加载，不再全图 roads.json | chunk JSON 含 road metadata；master 不加载全图道路 scene | `cargo test ... scene_writer` + E2E road metadata | done |
| M4 v2.3 分片抓取与导航索引 | bbox tiling、OSM 去重缓存、navigation_index | 单元测试覆盖切片/去重；索引含道路和建筑 | `cargo test ... tiled_fetch` + 生成外滩/上海缓存 | done |
| M5 v2.4 大上海 world package | 生成上海大范围项目并验证局部加载 | Godot import + E2E exit 0；README 记录命令和路径 | 上海项目生成 + Godot import/E2E | done |

## Plan Index

- [v2-world-streaming.md](v2-world-streaming.md)

## Traceability Matrix

| Req ID | PRD | v2 Plan | Unit/Integration Tests | E2E | Evidence | Status |
|---|---|---|---|---|---|---|
| REQ-0002-001 | PRD-0002 §REQ-0002-001 | v2-streaming §Step 1-4 | `scene_writer` master/manifest tests | 外滩 streaming E2E | 外滩 v6 `manifest_chunk_count=269`，`master.tscn` 不静态引用 chunk | done |
| REQ-0002-002 | PRD-0002 §REQ-0002-002 | v2-streaming §Step 1-4 | `world_streamer.gd` content tests | 外滩/上海 streaming E2E | 外滩 `loaded_initial=20`，上海 `loaded_initial=9`，移动后 chunk keys 变化 | done |
| REQ-0002-003 | PRD-0002 §REQ-0002-003 | v2-streaming §Step 1-4 | `tscn_writer` road chunk tests | road metadata E2E | 外滩/上海 `roads_node_exists=false`，道路 metadata 节点可读 | done |
| REQ-0002-004 | PRD-0002 §REQ-0002-004 | v2-streaming §Step 1-4 | `navigation_index` tests | metadata E2E | 上海 `navigation_index.json` 含 `445340` 条记录 | done |
| REQ-0002-005 | PRD-0002 §REQ-0002-005 | v2-streaming §Step 1-4 | bbox tiling + OSM dedupe tests | 上海生成命令 | 30 个 tile 全部命中 `E:\tmp\osm-godot-cache\shanghai` 缓存，合并 `3865075` 个 unique elements | done |
| REQ-0002-006 | PRD-0002 §REQ-0002-006 | v2-streaming §Step 5-6 | 全量 cargo test | 上海 Godot import/E2E | `E:\tmp\osm-godot-shanghai-city-v2-streaming-c512` import/E2E exit 0 | done |

## ECN Index

当前无 ECN。若施工中发现“大上海”必须缩小为样例 bbox，必须写 ECN，不得口头降级。

## DoD Hardness Gate

- [x] 每条需求均有二元验收：测试 exit code、文件存在、manifest 条目数、E2E 断言。
- [x] 每条需求均绑定验证命令或脚本。
- [x] 反作弊条款：不得只保留离线 chunk 文件却仍在 `master.tscn` 全量引用；测试必须检查 `master.tscn` 不含 `Chunk_*.tscn` ext_resource。
- [x] 反作弊条款：不得只切建筑不切道路；道路 metadata 必须在 chunk JSON 或 streaming 加载后的节点 meta 中可读。
- [x] 反作弊条款：不得只生成外滩；本轮必须生成上海大范围 world package，若网络/Overpass 阻塞必须记录失败原因和可复跑缓存策略。
- [x] Scope 明确排除车辆物理、路线规划、HUD、语音导航、服务端和数据库。

## Difference List

已关闭差异：

- `master.tscn` 不再静态引用全部 chunk，改由 `world_streamer.gd` 读取 `world_manifest.json`。
- 道路已写入 chunk JSON；不再生成或依赖全图 `mesh_data/roads.json`。
- Overpass 大 bbox 已支持 tiled fetch、cache 和 `(type,id)` 去重。
- 已生成 `navigation_index.json`，道路/建筑记录包含 chunk、center、bbox 和 OSM 元数据。
- 修复全上海运行时低 FPS 根因：`world_streamer.gd` 不再每帧全量扫描 manifest；`chunk_mesh_loader.gd` 按材质合批 mesh，同时保留轻量 OSM metadata marker。

新增发现：

- 整上海使用 `--chunk-size 128` 会产生十几万到二十多万个 chunk 文件，生成和 Godot import 都过慢；本机验收路径改为 `--chunk-size 512 --stream-radius 1`，实际输出 `33879` 个非空 chunk，E2E 仍只加载 player 附近最多 `3x3` 个 chunk。
- OSM 中存在异常建筑 `way 1371593537`，`building:levels=1235678911121415`；已在高度解析和 facade 细节生成处增加限幅测试，避免单个脏数据触发巨量 mesh 分配。
- 全上海 c512 初始视距内原始 element 仍很多：9 个 chunk 覆盖 `5402` 个 element；合批后实际 `MeshInstance3D=136`，streamer 稳态刷新 `avg_refresh_usec=7.05`。

验证证据：

- `cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture`：33 passed。
- `cargo test --target-dir E:\tmp\osm-godot-target element_processing::buildings -- --nocapture`：9 passed。
- 外滩 v6：`E:\tmp\osm-godot-shanghai-bund-v6-streaming`，Godot import exit 0，E2E exit 0。
- 整上海 c512：`E:\tmp\osm-godot-shanghai-city-v2-streaming-c512`，Godot import exit 0，E2E exit 0。
- 性能探针：`tools\godot_streaming_perf_probe.gd` 在外滩 v6 和整上海 c512 上 exit 0。
