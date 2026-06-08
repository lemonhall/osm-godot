# v4-building-inspection: F 键查看眼前建筑

## Goal

实现本地建筑信息查看：玩家按 `F` 时，系统从相机视线锥中选出最像“眼前”的已加载建筑 metadata marker，并显示名称与关键 OSM 属性，5 秒后自动隐藏。

## PRD Trace

- REQ-0004-001：F 键查看眼前建筑
- REQ-0004-002：视线锥选择规则可解释
- REQ-0004-003：HUD 信息卡 4-5 秒自动消失
- REQ-0004-004：全上海项目同步可用

## Scope

做：
- 在 `navigation_controller.gd` 中增加 inspect 输入、候选筛选、HUD 信息卡。
- 从已加载 scene tree 的 `osm_metadata` marker 读取建筑名称与属性。
- 在全上海工程 `E:\tmp\osm-godot-shanghai-city-v3-navigation-c512` 同步脚本。
- 增加 Rust 模板测试和 Godot E2E。

不做：
- 不新增联网查询。
- 不查询未加载 chunk。
- 不做鼠标点击选择、详情历史、可滚动属性面板。
- 不给建筑 mesh 增加碰撞体。

## Acceptance

1. `cargo test --target-dir E:\tmp\osm-godot-target building_inspection` 通过。
2. 生成脚本包含 `@export var inspect_key := KEY_F`、`inspect_display_seconds := 5.0`、`BuildingInspectPanel`、`_find_looked_at_building()`、`_show_building_inspection()`。
3. 选择逻辑只接受建筑 metadata，且候选必须在 camera 前方和视线锥范围内。
4. Godot E2E 构造可控 metadata marker，触发 inspect 后面板显示 marker 名称，等待超过 5 秒后隐藏。
5. 全上海工程 Godot 4.6 import exit 0，且 `tools\godot_building_inspection_e2e.gd` exit 0。

## Files

- `docs/prd/PRD-0004-building-inspection.md`
- `docs/plan/v4-index.md`
- `docs/plan/v4-building-inspection.md`
- `src/scene_writer/mod.rs`
- `tools/godot_building_inspection_e2e.gd`
- `E:\tmp\osm-godot-shanghai-city-v3-navigation-c512\scripts\navigation_controller.gd`

## Steps

1. **Red**：新增 `building_inspection` Rust 测试，断言导航控制器模板包含 F 键、视线锥选择、HUD panel 和 5 秒自动隐藏。状态：done。
2. **Red verify**：运行 `cargo test --target-dir E:\tmp\osm-godot-target building_inspection`，预期失败，失败点为缺少 inspect 字段/函数。状态：done。
3. **Green**：实现 `navigation_controller.gd` 模板中的 inspect key、camera lookup、metadata candidate scan、HUD panel、timer hide。状态：done。
4. **Green verify**：运行 `cargo test --target-dir E:\tmp\osm-godot-target building_inspection`，预期通过。状态：done。
5. **Shanghai sync**：把同等脚本能力同步到全上海工程，并运行 Godot import。状态：done。
6. **E2E**：新增并运行 `tools\godot_building_inspection_e2e.gd`，预期 marker 名称可见、5 秒后隐藏。状态：done。
7. **Regression**：运行全量 `cargo test --target-dir E:\tmp\osm-godot-target`，以及已有导航/自动巡航关键 E2E。状态：done。
8. **Review**：更新 `v4-index.md` trace/evidence/status，运行 `git diff --check` 与乱码扫描。状态：done。

## Execution Evidence

- Red：`cargo test --target-dir E:\tmp\osm-godot-target building_inspection` 曾因缺少 `inspect_key` 失败。
- Green：`cargo test --target-dir E:\tmp\osm-godot-target building_inspection` passed。
- Regression：`cargo test --target-dir E:\tmp\osm-godot-target` passed，91 passed / 1 ignored。
- Shanghai import：Godot 4.6 headless import exit 0。
- Building inspect E2E：`INSPECT_E2E title=塔山测试楼`、`no_distractor=true`、`hidden_after_timer=true`。
- Auto-run regression E2E：`AUTORUN_E2E moved=41.4922485351563`，exit 0。

## Risks

- **建筑没有碰撞体**：使用 metadata marker + 视线锥筛选，不依赖 physics raycast。
- **marker 可能不在建筑几何中心**：评分先看离视线中心的垂直距离，再看前向距离，降低误选旁边建筑概率。
- **属性太多遮挡画面**：只显示名称和少量常见字段，5 秒自动隐藏。
