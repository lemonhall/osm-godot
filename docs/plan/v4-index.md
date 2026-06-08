# v4: 视线建筑信息查看

关联 PRD：[PRD-0004: 视线建筑信息查看](../prd/PRD-0004-building-inspection.md)

## Vision

玩家按 `F` 即可查看眼前已加载建筑的本地 OSM 名称与属性，信息卡停留约 5 秒后消失，不打断导航、自动巡航或 FPS 漫游。

## Milestones

| Milestone | Scope | DoD | Verification | Status |
|---|---|---|---|---|
| M1 文档与验收冻结 | PRD-0004、v4 index、v4 计划 | 每条 REQ 有计划、测试或 E2E；DoD 可二元判定 | `git diff --text -- docs` + 编码扫描 | done |
| M2 红测 | 模板测试和 Godot E2E 脚本先失败 | 缺实现时 `building_inspection` 测试失败，失败原因指向缺失能力 | `cargo test ... building_inspection` | done |
| M3 实现 | F 键、视线锥建筑选择、HUD 自动隐藏 | 仓库模板和全上海工程均具备能力 | `cargo test ... building_inspection` + Godot import | done |
| M4 E2E 与回顾 | 全上海工程运行建筑查看 E2E | E2E exit 0；差异列表更新 | `tools\godot_building_inspection_e2e.gd` | done |

## Plan Index

- [v4-building-inspection.md](v4-building-inspection.md)

## Traceability

| Req ID | PRD | Plan | Tests / Commands | Evidence | Status |
|---|---|---|---|---|---|
| REQ-0004-001 | PRD-0004 §REQ-0004-001 | v4-building-inspection §Step 1-4 | `building_inspection_controller_has_f_key_lookup_and_hud` + Godot E2E | `cargo test --target-dir E:\tmp\osm-godot-target building_inspection` passed；E2E title=`塔山测试楼` | done |
| REQ-0004-002 | PRD-0004 §REQ-0004-002 | v4-building-inspection §Step 1-4 | script content assertions for cone scoring + Godot distractor marker | Rust 断言包含 `inspect_cone_degrees`、`perpendicular`；E2E `no_distractor=true` | done |
| REQ-0004-003 | PRD-0004 §REQ-0004-003 | v4-building-inspection §Step 1-4 | `BuildingInspectPanel` E2E auto-hide | E2E `shown=true`、`hidden_after_timer=true` | done |
| REQ-0004-004 | PRD-0004 §REQ-0004-004 | v4-building-inspection §Step 5-6 | Godot import + E2E on Shanghai project | 全上海工程 import exit 0；`godot_building_inspection_e2e.gd` exit 0 | done |

## ECN Index

- None.

## Differences

- 已满足：F 键建筑查看、视线锥选择、HUD 约 5 秒隐藏、全上海工程同步。
- 已知边界：只查询当前已加载 chunk 中的 `osm_metadata` marker；不做遮挡判断，不查询未加载区域，不调用外部服务。这与 PRD 非目标一致，不进入下一版差异。

## Verification Evidence

```powershell
cargo test --target-dir E:\tmp\osm-godot-target building_inspection
# result: ok. 1 passed

cargo test --target-dir E:\tmp\osm-godot-target
# result: ok. 91 passed; 0 failed; 1 ignored

& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v3-navigation-c512 --import --quit
# exit code: 0

& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v3-navigation-c512 --script E:\development\osm-godot\tools\godot_building_inspection_e2e.gd
# INSPECT_E2E shown=true
# INSPECT_E2E title=塔山测试楼
# INSPECT_E2E no_distractor=true
# INSPECT_E2E hidden_after_timer=true

& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v3-navigation-c512 --script E:\development\osm-godot\tools\godot_navigation_autorun_e2e.gd
# AUTORUN_E2E moved=41.4922485351563
# exit code: 0
```

## DoD Hardness Check

- [x] 每条 DoD 均可二元判定或有数字阈值。
- [x] 每条 DoD 均绑定验证命令。
- [x] 反作弊条款：不得只显示固定假数据；E2E 必须从 `osm_metadata` marker 读取名称和属性。
- [x] Scope 明确不做鼠标点击、外部查询、未加载 chunk 查询或持久历史。
