# v6: OSM 植被与绿地生成

关联 PRD：[PRD-0006: OSM 植被与绿地生成](../prd/PRD-0006-osm-vegetation.md)

## Vision

上海等大范围 Godot 城市应从 OSM 面状绿地中生成可见的草地、林地、树木和灌木；单棵树应有多种稳定形态，避免绿植稀少和形状单调。

## Milestones

| Milestone | Scope | DoD | Verification | Status |
|---|---|---|---|---|
| M1 文档与验收冻结 | PRD-0006、v6 index、v6 计划 | 每条 REQ 有计划、测试或 E2E；DoD 可二元判定 | `git diff --check -- docs` + 编码扫描 | done |
| M2 红测 | vegetation 分类、撒布、树形、E2E 契约先失败 | `cargo test ... vegetation` 因缺实现失败 | `cargo test --target-dir E:\tmp\osm-godot-target vegetation` | todo |
| M3 实现 | 面状绿地 patch、确定性撒树/灌木、多树形 | 单元/脚本测试通过，输出包含多类 vegetation mesh | `cargo test ... vegetation` | todo |
| M4 E2E 与回归 | v6 样本工程 Godot E2E + 500m FPS 巡航 + 全量 Rust 回归 | E2E exit 0；`walk_distance >= 500.0`；`avg_fps >= 55.0`；`min_fps >= 30.0`；全量 cargo test 通过 | Godot E2E + `cargo test` | todo |
| M5 文档回顾与交付 | README/AGENTS 如需更新，v6 evidence 回写 | 追溯矩阵无断链；差异列表明确；commit+push | `git diff --check` | todo |

## Plan Index

- [v6-osm-vegetation.md](v6-osm-vegetation.md)

## Traceability

| Req ID | PRD | Plan | Tests / Commands | Evidence | Status |
|---|---|---|---|---|---|
| REQ-0006-001 | PRD-0006 §REQ-0006-001 | v6-osm-vegetation §Step 1-4 | `vegetation_area_classifies_osm_green_polygons` | pending | todo |
| REQ-0006-002 | PRD-0006 §REQ-0006-002 | v6-osm-vegetation §Step 1-6 | `vegetation_area_writes_ground_patch_mesh` | pending | todo |
| REQ-0006-003 | PRD-0006 §REQ-0006-003 | v6-osm-vegetation §Step 1-6 | `vegetation_scatter_is_deterministic_and_capped` | pending | todo |
| REQ-0006-004 | PRD-0006 §REQ-0006-004 | v6-osm-vegetation §Step 1-6 | `tree_generation_has_multiple_profiles` | pending | todo |
| REQ-0006-005 | PRD-0006 §REQ-0006-005 | v6-osm-vegetation §Step 7-9 | `tools/godot_vegetation_e2e.gd` + full `cargo test` | pending | todo |
| REQ-0006-006 | PRD-0006 §REQ-0006-006 | v6-osm-vegetation §Step 7-9 | `tools/godot_vegetation_e2e.gd` FPS walk | pending | todo |

## ECN Index

- None.

## Plan Changes

- 2026-06-09：在进入 TDD Red 前补充用户硬性要求：植被不得拖累 FPS；v6 E2E 必须让 player 自动移动约 500m 并断言平均 FPS 与最低 FPS。

## Differences

- Pending until implementation review.

## Verification Evidence

```powershell
rg -n "REQ-0006-|DoD|Acceptance" docs\prd\PRD-0006-osm-vegetation.md docs\plan\v6-index.md docs\plan\v6-osm-vegetation.md
# result: REQ-0006-001..006 all traceable from PRD to v6 plan

rg -a -n <encoding-sentinel-regex> docs\prd\PRD-0006-osm-vegetation.md docs\plan\v6-index.md docs\plan\v6-osm-vegetation.md
# result: no matches

git diff --check -- docs\prd\PRD-0006-osm-vegetation.md docs\plan\v6-index.md docs\plan\v6-osm-vegetation.md
# result: exit 0
```

## DoD Hardness Check

- [x] 每条 DoD 均可二元判定：分类、mesh 名称、材质、实例数量、E2E exit code、500m 移动距离和 FPS 阈值。
- [x] 每条 DoD 均绑定验证命令。
- [x] 反作弊条款：不能只改地表颜色；必须从至少一种 OSM 面状绿地生成 `VegetationGround_*` 和 3 个以上具体植被实例；必须存在至少三类植被形态；必须通过 500m 自动移动 FPS E2E。
- [x] Scope 明确不做 multipolygon relation、真实树种、外部 asset、建筑/道路布尔避让和全上海重生成。
