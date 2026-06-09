# v5: 建筑中文门匾

关联 PRD：[PRD-0005: 建筑中文门匾](../prd/PRD-0005-building-plaques.md)

## Vision

流式加载的 Godot 城市中，带正式中文名称的建筑会自动生成白底黑字门匾：普通建筑为门旁竖牌，店铺/服务类为门上横匾；没有正式中文名的建筑不生成牌匾。

## Milestones

| Milestone | Scope | DoD | Verification | Status |
|---|---|---|---|---|
| M1 文档与验收冻结 | PRD-0005、v5 index、v5 计划 | 每条 REQ 有计划、测试或 E2E；DoD 可二元判定 | `git diff --text -- docs` + 编码扫描 | done |
| M2 红测 | metadata 与 loader 牌匾契约先失败 | `building_plaque` 测试因缺字段/缺函数失败 | `cargo test ... building_plaque` | done |
| M3 实现 | 中文字段保留、去重、竖/横牌匾生成 | 单元/脚本测试通过；无英文假牌 | `cargo test ... building_plaque` | done |
| M4 E2E 与回归 | Godot E2E + 全量 Rust 回归 | E2E exit 0；全量 cargo test 通过 | Godot E2E + `cargo test` | done |
| M5 文档回顾 | README/AGENTS 如需更新，v5 evidence 回写 | 差异列表明确，追溯矩阵无断链 | `git diff --check` | done |

## Plan Index

- [v5-building-plaques.md](v5-building-plaques.md)

## Traceability

| Req ID | PRD | Plan | Tests / Commands | Evidence | Status |
|---|---|---|---|---|---|
| REQ-0005-001 | PRD-0005 §REQ-0005-001 | v5-building-plaques §Step 1-4 | `building_plaque_metadata_preserves_chinese_name_tags` + `building_plaque_filter_preserves_chinese_name_tags` | `cargo test ... building_plaque` passed；`name:zh` / `official_name:zh` / `operator:zh` 保留 | done |
| REQ-0005-002 | PRD-0005 §REQ-0005-002 | v5-building-plaques §Step 1-4 | `building_plaque_chunk_loader_filters_chinese_names_and_deduplicates` + Godot E2E | `PLAQUE_E2E plaque_count=1`，同 `osm_id` 去重 | done |
| REQ-0005-003 | PRD-0005 §REQ-0005-003 | v5-building-plaques §Step 1-4 | `building_plaque_chunk_loader_has_vertical_and_storefront_layouts` + Godot E2E | 白底黑字 `Label3D` + 竖排文本 `建|筑|科|学|院` | done |
| REQ-0005-004 | PRD-0005 §REQ-0005-004 | v5-building-plaques §Step 1-4 | chunk loader script assertions | 牌匾逻辑位于 `chunk_mesh_loader.gd`，随 chunk 子树加载/释放 | done |
| REQ-0005-005 | PRD-0005 §REQ-0005-005 | v5-building-plaques §Step 5-7 | `tools/godot_building_plaques_e2e.gd` | 最小工程与全上海工程 E2E 均 exit 0 | done |

## ECN Index

- None.

## Differences

- 已满足：中文正式名筛选、parser/metadata 保留中文名称字段、同一建筑去重、普通竖牌、店铺横匾路径、白底黑字 `Label3D`、chunk 生命周期内生成。
- 已同步：`E:\tmp\osm-godot-shanghai-city-v3-navigation-c512\scripts\chunk_mesh_loader.gd` 已更新并通过牌匾 E2E。
- 已知边界：现有全上海 mesh 数据是旧生成结果，只有已存在于 `name` / `official_name` 的纯中文名称会立即挂牌；`name:zh` 等新保留字段需要未来重新生成全上海工程后才会进入 mesh_data。

## Verification Evidence

```powershell
cargo test --target-dir E:\tmp\osm-godot-target building_plaque
# result: ok. 4 passed

cargo test --target-dir E:\tmp\osm-godot-target
# result: ok. 95 passed; 0 failed; 1 ignored

cargo run --target-dir E:\tmp\osm-godot-target -- --file E:\tmp\osm-godot-empty-osm.json --bbox "34.2160,108.9550,34.2210,108.9620" --output-dir E:\tmp\osm-godot-plaque-e2e --chunk-size 128
# result: exit 0, generated E:\tmp\osm-godot-plaque-e2e

& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-plaque-e2e --script E:\development\osm-godot\tools\godot_building_plaques_e2e.gd
# PLAQUE_E2E plaque_count=1
# PLAQUE_E2E chinese_count=1
# PLAQUE_E2E english_count=0
# PLAQUE_E2E label_text=建|筑|科|学|院

& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v3-navigation-c512 --script E:\development\osm-godot\tools\godot_building_plaques_e2e.gd
# PLAQUE_E2E plaque_count=1
# PLAQUE_E2E chinese_count=1
# PLAQUE_E2E english_count=0

git diff --check
# result: exit 0
```

## DoD Hardness Check

- [x] 每条 DoD 均可二元判定或有明确输出。
- [x] 每条 DoD 均绑定验证命令。
- [x] 反作弊条款：不得从英文/类别合成中文名；没有正式中文名不挂牌；同一 `osm_id` 最多一个牌匾。
- [x] Scope 明确不做真实门洞识别、外部查询、多入口多牌、全图静态牌匾。
