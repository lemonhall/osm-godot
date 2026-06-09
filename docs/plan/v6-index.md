# v6: OSM 植被与绿地生成

关联 PRD：[PRD-0006: OSM 植被与绿地生成](../prd/PRD-0006-osm-vegetation.md)

## Vision

上海等大范围 Godot 城市应从 OSM 面状绿地中生成可见的草地、林地、树木和灌木；单棵树应有多种稳定形态，避免绿植稀少和形状单调。

## Milestones

| Milestone | Scope | DoD | Verification | Status |
|---|---|---|---|---|
| M1 文档与验收冻结 | PRD-0006、v6 index、v6 计划 | 每条 REQ 有计划、测试或 E2E；DoD 可二元判定 | `git diff --check -- docs` + 编码扫描 | done |
| M2 红测 | vegetation 分类、撒布、树形、E2E 契约先失败 | `cargo test ... vegetation` 因缺实现失败 | `cargo test --target-dir E:\tmp\osm-godot-target vegetation` | done |
| M3 实现 | 面状绿地 patch、确定性撒树/灌木、多树形 | 单元/脚本测试通过，输出包含多类 vegetation mesh | `cargo test ... vegetation` | done |
| M4 E2E 与回归 | v6 样本工程 Godot E2E + 500m FPS 巡航 + 全量 Rust 回归 | E2E exit 0；`walk_distance >= 500.0`；`avg_fps >= 55.0`；`min_fps >= 30.0`；全量 cargo test 通过 | Godot E2E + `cargo test` | done |
| M5 文档回顾与交付 | README/AGENTS 如需更新，v6 evidence 回写 | 追溯矩阵无断链；差异列表明确；commit+push | `git diff --check` | done |

## Plan Index

- [v6-osm-vegetation.md](v6-osm-vegetation.md)

## Traceability

| Req ID | PRD | Plan | Tests / Commands | Evidence | Status |
|---|---|---|---|---|---|
| REQ-0006-001 | PRD-0006 §REQ-0006-001 | v6-osm-vegetation §Step 1-4 | `vegetation_area_classifies_osm_green_polygons` | forest/park/scrub 生成 `VegetationGround_*`，parking 不生成 | done |
| REQ-0006-002 | PRD-0006 §REQ-0006-002 | v6-osm-vegetation §Step 1-6 | `vegetation_area_writes_ground_patch_mesh` | garden 输出 `VegetationGround_201`，材质为 `TerrainGrass` / `TreeLeaves` | done |
| REQ-0006-003 | PRD-0006 §REQ-0006-003 | v6-osm-vegetation §Step 1-6 | `vegetation_scatter_is_deterministic_and_capped` | woodland 撒布稳定，实例数 `>=3` 且 `<=48` | done |
| REQ-0006-004 | PRD-0006 §REQ-0006-004 | v6-osm-vegetation §Step 1-6 | `vegetation_tree_generation_has_multiple_profiles` | 输出 `VegetationTree_*`、`VegetationConifer_*`、`VegetationShrub_*`，不再输出旧 `Tree_*` | done |
| REQ-0006-005 | PRD-0006 §REQ-0006-005 | v6-osm-vegetation §Step 7-9 | `tools/godot_vegetation_e2e.gd` + full `cargo test` | E2E `mesh_ground=4`、`runtime_markers=1`、full test `99 passed / 1 ignored` | done |
| REQ-0006-006 | PRD-0006 §REQ-0006-006 | v6-osm-vegetation §Step 7-9 | `tools/godot_vegetation_e2e.gd` FPS walk | `walk_distance=500.0`、`avg_fps=62.1539`、`min_fps=44.9438` | done |

## ECN Index

- None.

## Plan Changes

- 2026-06-09：在进入 TDD Red 前补充用户硬性要求：植被不得拖累 FPS；v6 E2E 必须让 player 自动移动约 500m 并断言平均 FPS 与最低 FPS。

## Differences

- 已满足：OSM 面状绿地分类、绿地 ground patch、多 profile 植被实例、确定性撒布与上限、Godot runtime streaming 验证、500m FPS 巡航。
- 已知边界：v6 不处理 multipolygon relation，不做道路/建筑布尔避让，不重新生成完整全上海工程；这些保持为后续版本空间。

## Verification Evidence

```powershell
rg -n "REQ-0006-|DoD|Acceptance" docs\prd\PRD-0006-osm-vegetation.md docs\plan\v6-index.md docs\plan\v6-osm-vegetation.md
# result: REQ-0006-001..006 all traceable from PRD to v6 plan

rg -a -n <encoding-sentinel-regex> docs\prd\PRD-0006-osm-vegetation.md docs\plan\v6-index.md docs\plan\v6-osm-vegetation.md
# result: no matches

git diff --check -- docs\prd\PRD-0006-osm-vegetation.md docs\plan\v6-index.md docs\plan\v6-osm-vegetation.md
# result: exit 0

cargo test --target-dir E:\tmp\osm-godot-target vegetation
# initial red: 0 passed; 4 failed
# final green: 4 passed

cargo run --target-dir E:\tmp\osm-godot-target -- --file E:\tmp\osm-godot-vegetation-osm.json --bbox "31.2290,121.4820,31.2455,121.5100" --output-dir E:\tmp\osm-godot-vegetation-v6-e2e --chunk-size 128 --stream-radius 1
# result: Vegetation areas: 4; Total scene elements: 95; Wrote 26 non-empty chunk scenes

& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-vegetation-v6-e2e --import --quit
# result: exit 0

& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-vegetation-v6-e2e --script E:\development\osm-godot\tools\godot_vegetation_e2e.gd
# result: mesh_ground=4; mesh_tree=20; mesh_conifer=7; mesh_shrub=37; runtime_markers=1; runtime_batches=9; walk_distance=500.0; avg_fps=62.1539; min_fps=44.9438

cargo test --target-dir E:\tmp\osm-godot-target
# result: 99 passed; 1 ignored
```

## DoD Hardness Check

- [x] 每条 DoD 均可二元判定：分类、mesh 名称、材质、实例数量、E2E exit code、500m 移动距离和 FPS 阈值。
- [x] 每条 DoD 均绑定验证命令。
- [x] 反作弊条款：不能只改地表颜色；必须从至少一种 OSM 面状绿地生成 `VegetationGround_*` 和 3 个以上具体植被实例；必须存在至少三类植被形态；必须通过 500m 自动移动 FPS E2E。
- [x] Scope 明确不做 multipolygon relation、真实树种、外部 asset、建筑/道路布尔避让和全上海重生成。
