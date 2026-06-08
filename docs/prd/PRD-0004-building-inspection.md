# PRD-0004: 视线建筑信息查看

## Vision

玩家在流式加载的 Godot 城市里漫游时，可以像真实导航或城市导览一样，快速知道“我现在看着的这栋楼是什么”。按一次键后，游戏只使用本地已加载的 OSM 元数据，在屏幕上短暂显示建筑名称和关键属性；玩家不需要打开搜索面板，也不需要调用任何外部服务。

## Context

当前 chunk loader 已为道路和建筑创建轻量 metadata marker，并把原始 OSM 字段放入 `osm_metadata`。建筑 mesh 本身没有碰撞体，不能可靠通过物理 raycast 命中建筑表面。因此本需求把“眼前建筑”定义为：相机前方一定距离内、落在视线锥内、且离视线中心最近的已加载建筑 metadata marker。

## Requirements

### REQ-0004-001: F 键查看眼前建筑

- **Motivation**：玩家在城市中漫游时，需要快速识别眼前建筑。
- **Scope**：按 `F` 从当前 camera 位置和朝向计算候选；只选择 `osm_kind=building` 或带 `building` 字段的 metadata marker。
- **Non-goals**：不做鼠标点击选择；不查询未加载 chunk；不联网查询。
- **Acceptance**：Godot E2E 构造一条相机前方建筑 metadata marker，调用 inspect 后返回 `found`，并显示该 marker 的名称。

### REQ-0004-002: 视线锥选择规则可解释

- **Motivation**：需求中的“眼前”是模糊概念，必须固定成稳定规则，避免随机选中旁边建筑。
- **Scope**：候选必须在相机前方、距离不超过配置上限、且与视线方向夹角不超过配置视线锥；评分优先垂直于视线的偏移距离，其次沿视线距离。
- **Non-goals**：不承诺遮挡判断；不要求建筑真实轮廓参与命中。
- **Acceptance**：单元/脚本内容测试证明存在 `inspect_max_distance`、`inspect_cone_degrees`、`_find_looked_at_building()` 和基于 dot/perpendicular 的评分逻辑。

### REQ-0004-003: HUD 信息卡 4-5 秒自动消失

- **Motivation**：信息展示应短暂、可读，不打断驾驶和漫游。
- **Scope**：显示建筑名、建筑类型和常见 OSM 属性；默认 `inspect_display_seconds=5.0`，到时自动隐藏。
- **Non-goals**：不做可滚动详情页；不做历史记录；不阻塞玩家控制。
- **Acceptance**：Godot E2E 证明 inspect 后 `BuildingInspectPanel` 可见，等待超过 5 秒后自动隐藏。

### REQ-0004-004: 全上海项目同步可用

- **Motivation**：用户当前主要使用 `E:\tmp\osm-godot-shanghai-city-v3-navigation-c512`。
- **Scope**：仓库模板和该全上海工程的 `navigation_controller.gd` 均具备 F 键信息查看能力。
- **Non-goals**：不重新生成全上海工程；不改变已有导航和自动巡航数据文件。
- **Acceptance**：Godot 4.6 对全上海工程 import 成功；建筑信息查看 E2E 在该工程 exit 0。

## Verification Commands

```powershell
cargo test --target-dir E:\tmp\osm-godot-target building_inspection
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v3-navigation-c512 --import --quit
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-city-v3-navigation-c512 --script E:\development\osm-godot\tools\godot_building_inspection_e2e.gd
```
