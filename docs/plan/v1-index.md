# v1: Arnis-style Godot 建筑增强

## Vision

关联 PRD：[PRD-0001: Arnis-style Godot 建筑生成](../prd/PRD-0001-arnis-style-buildings.md)

本轮目标是把“OSM 素体盒子”升级为“可分类、可辨识、可漫游检查”的 Godot 城市建筑。实现必须保留当前 mesh 管线、道路独立场景、地面、光照和 player，不引入 voxel 化重写。

## Milestones

| Milestone | Scope | DoD | Verification | Status |
|---|---|---|---|---|
| M1 文档与验收冻结 | PRD、v1 index、v1 计划、追溯矩阵 | 所有 Req ID 都有计划、测试或 E2E 验证入口；DoD 可二元判定 | `git diff --text -- docs` + 编码扫描 | done |
| M2 红测落地 | 为分类、材质、屋顶、立面、屋顶设备添加失败测试 | 新测试在实现前失败，失败原因对应缺失能力 | `cargo test --target-dir E:\tmp\osm-godot-target element_processing::buildings -- --nocapture` | done |
| M3 绿测实现 | 实现 Arnis-style facade grammar | M2 测试全绿，既有建筑测试不回退 | `cargo test --target-dir E:\tmp\osm-godot-target element_processing::buildings -- --nocapture` | done |
| M4 集成与生成 | 更新场景写出、材质、README，生成上海外滩 v4 项目 | 项目生成成功，Godot 4.6 import 无迁移/无崩溃 | `cargo run ... --output-dir E:\tmp\osm-godot-shanghai-bund-v4-arnis-buildings` + Godot import | done |
| M5 E2E 与回顾 | Godot headless E2E 验证主场景可运行 | E2E 通过；差异列表更新；完成通知发送 | Godot `--script tools\godot_player_e2e.gd` | done |

## Plan Index

- [v1-arnis-style-buildings.md](v1-arnis-style-buildings.md)

## Traceability Matrix

| Req ID | PRD | v1 Plan | Unit/Integration Tests | E2E | Evidence | Status |
|---|---|---|---|---|---|---|
| REQ-0001-001 | PRD-0001 §REQ-0001-001 | v1-arnis §Step 1-4 | `src/element_processing/buildings.rs` 分类测试 | 上海外滩生成统计 | `cargo test ... element_processing::buildings`：8 passed；外滩 v4 建筑 `1105` | done |
| REQ-0001-002 | PRD-0001 §REQ-0001-002 | v1-arnis §Step 1-4 | `src/element_processing/buildings.rs` 材质测试 | 生成材质文件检查 | `cargo test ... element_processing::buildings`：8 passed；材质资源写出通过 Godot import | done |
| REQ-0001-003 | PRD-0001 §REQ-0001-003 | v1-arnis §Step 1-4 | `src/scene_writer/geometry.rs` / `buildings.rs` 屋顶测试 | Godot import | `cargo test ...`：66 passed, 1 ignored；Godot import exit 0 | done |
| REQ-0001-004 | PRD-0001 §REQ-0001-004 | v1-arnis §Step 1-4 | `make_building_detail_meshes` 细节测试 | 上海外滩视觉/E2E 统计 | 外滩 v4 场景元素 `6981`；Godot E2E exit 0 | done |
| REQ-0001-005 | PRD-0001 §REQ-0001-005 | v1-arnis §Step 1-4 | 屋顶设备/烟囱/标记测试 | Godot import | `cargo test ... element_processing::buildings`：8 passed；Godot import exit 0 | done |
| REQ-0001-006 | PRD-0001 §REQ-0001-006 | v1-arnis §Step 5-6 | `scene_writer` 回归测试 | `tools/godot_player_e2e.gd` | `scene_writer`：28 passed；E2E `road_mesh_count=1613`、`normal_all_collision_xz_move=21.0000648498535` | done |
| REQ-0001-007 | PRD-0001 §REQ-0001-007 | v1-arnis §Step 1-6 | `scene_writer` mesh metadata 测试 | `tools/godot_player_e2e.gd` 元数据断言 | E2E `osm_meta_node=Highway_166125929`、`osm_meta_kind=road`、`osm_meta_id=166125929` | done |

## ECN Index

当前无 ECN。导航元数据需求在实现前已补入 PRD 与计划，未改变已冻结后的验收口径。

## DoD Hardness Gate

- [x] 每条需求均有二元或可量化验收：测试通过、mesh 非空、材质枚举命中、E2E 退出码为 0。
- [x] 每条需求均绑定验证命令或测试入口。
- [x] 反作弊条款：不得只新增类别枚举而不影响 `BuildingStyle`、屋顶 mesh 或细节 mesh；测试必须检查输出材质/mesh/类别，不只检查函数可调用。
- [x] 导航元数据反作弊条款：不得只把 OSM 名称塞进节点名；必须在 JSON 和 Godot `meta` 中能读取 `osm_id` 与 `osm_kind`。
- [x] Scope 明确排除 voxel 化、外部 3D 模型、室内和地标手工建模。

## Difference List

本轮回顾：

- 已满足：建筑分类、材质优先级、屋顶语法、类别化立面、屋顶装饰、道路/建筑 OSM 元数据、上海外滩 v4 生成、Godot 4.6 import、主场景 E2E 均有自动化证据。
- 已满足：新工程路径为 `E:\tmp\osm-godot-shanghai-bund-v4-arnis-buildings\project.godot`。
- 新增发现：Godot `set_meta` 的单独 key 不能包含 `:`，因此 loader 同时写入原始 `osm_metadata` 字典和安全化后的单独 meta key，例如 `addr_housenumber`、`building_levels`。
- 未满足：本轮不做路线规划、驾车物理、HUD、语音导航或真实地标模型导入；这些仍按 PRD Non-goals 留到后续版本。
