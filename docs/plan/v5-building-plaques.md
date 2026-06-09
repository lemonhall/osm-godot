# v5-building-plaques: 中文门匾

## Goal

实现流式 chunk 内的建筑中文门匾：从 OSM metadata 中筛选正式中文名称，普通建筑生成门旁竖牌，店铺/服务类生成门上横匾，并确保同一建筑最多一个牌匾。

## PRD Trace

- REQ-0005-001：只给正式中文名称建筑挂牌
- REQ-0005-002：每个建筑最多一个牌匾
- REQ-0005-003：普通建筑竖牌，店铺类横匾
- REQ-0005-004：牌匾随 chunk streaming 生命周期加载/卸载
- REQ-0005-005：端到端验证

## Scope

做：
- 扩展 OSM metadata 保留 `name:zh`、`official_name:zh`、`alt_name:zh`、`brand:zh`、`operator:zh` 等中文名称字段。
- 在 `chunk_mesh_loader.gd` 模板中创建 `BuildingPlaque` 节点，包含白色背景和黑色 `Label3D`。
- 按 `osm_id` 去重，避免 wall/roof/window 等多 mesh 重复挂牌。
- 根据店铺/服务类 metadata 选择横匾，否则生成单字换行竖牌。
- 新增 Godot E2E 脚本验证中文生成、英文过滤和去重。

不做：
- 不翻译英文名。
- 不从 `building` / `amenity` 类别合成中文名。
- 不识别真实门洞或多入口。
- 不把牌匾静态写进 `master.tscn`。
- 不联网查询建筑名称。

## Acceptance

1. `cargo test --target-dir E:\tmp\osm-godot-target building_plaque` 通过。
2. metadata 测试证明 `name:zh` / `official_name:zh` 等字段被写入建筑 metadata。
3. loader 脚本包含中文名筛选、CJK 判断、`plaque_building_ids` 去重、`BuildingPlaque`、`Label3D`、白色背景、黑色文字。
4. loader 脚本存在竖牌和横匾路径，店铺类 metadata 走横匾。
5. Godot E2E 证明中文名建筑生成牌匾，英文名建筑不生成，同一 `osm_id` 不重复。
6. `cargo test --target-dir E:\tmp\osm-godot-target` 通过。

## Files

- `docs/prd/PRD-0005-building-plaques.md`
- `docs/plan/v5-index.md`
- `docs/plan/v5-building-plaques.md`
- `src/element_processing/mod.rs`
- `src/osm_parser.rs`
- `src/scene_writer/mod.rs`
- `tools/godot_building_plaques_e2e.gd`
- `README.md` / `AGENTS.md`（如 CLI/输出说明需要同步）
- `E:\tmp\osm-godot-shanghai-city-v3-navigation-c512\scripts\chunk_mesh_loader.gd`

## Steps

1. **Red metadata**：新增 metadata 测试，断言中文命名字段保留。状态：done。
2. **Red parser**：新增 parser 过滤测试，断言 `name:zh` / `operator:zh` 不再被前缀过滤。状态：done。
3. **Red loader**：新增 chunk loader 脚本内容测试，断言中文过滤、去重、竖/横牌匾构建函数存在。状态：done。
4. **Red verify**：运行 `cargo test --target-dir E:\tmp\osm-godot-target building_plaque`，预期失败，失败原因指向缺少字段/函数。状态：done。
5. **Green implementation**：扩展 parser 白名单、metadata key，更新 chunk loader 模板，创建牌匾节点并去重。状态：done。
6. **Green verify**：运行 `cargo test --target-dir E:\tmp\osm-godot-target building_plaque`，预期通过。状态：done。
7. **E2E**：新增并运行 `tools\godot_building_plaques_e2e.gd`，验证中文生成、英文过滤、去重。状态：done。
8. **Shanghai sync**：同步 `chunk_mesh_loader.gd` 到全上海导航工程，并运行同一牌匾 E2E。状态：done。
9. **Regression**：运行 `cargo test --target-dir E:\tmp\osm-godot-target` 和 `git diff --check`。状态：done；`cargo test` 95 passed / 1 ignored，`git diff --check` exit 0。
10. **Review**：回写 v5 evidence、差异列表、必要 README/AGENTS 更新。状态：done。

## Risks

- **Godot 默认字体中文覆盖不稳定**：先使用 `Label3D` 默认字体，E2E 断言节点和文本，不把字体嵌入作为 v5 范围。
- **真实门位置不可得**：使用 wall mesh 外轮廓估算门面边；普通建筑门旁、店铺门上是近似布局。
- **重复 metadata marker**：以 `osm_id` 去重，保证同一建筑最多一个牌匾。
- **英文或拼音污染**：中文筛选必须包含 CJK 字符，且不能从英文或类别合成。
