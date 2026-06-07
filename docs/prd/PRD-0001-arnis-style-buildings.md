# PRD-0001: Arnis-style Godot 建筑生成

## Vision

生成城市时，Godot 里看到的建筑不再只是同质化盒子，而是能根据 OSM 标签推断用途、材质、屋顶和立面语言，形成接近 Arnis/Minecraft 演示中那种“用素体组合出丰富建筑”的视觉结果。用户在 FPS 漫游或俯视检查时，至少能稳定分辨住宅、商业、工业、公共/宗教、高层玻璃塔楼和历史风格建筑，并能看到道路、地面、光照与建筑共同组成完整城市。

## Background

当前仓库已经有基础建筑分类和若干立面细节，但视觉仍偏“盒子”：类别较粗，材质和屋顶规则有限，OSM 的 `building:*`、`roof:*`、`amenity` 等标签没有被充分用于生成更多可辨认的建筑特征。

`refs/arnis` 的核心价值不是 Minecraft 本身，而是它把 OSM 素体轮廓变成更丰富建筑的规则链：

- 从 OSM 标签推断建筑类别和生命周期状态。
- 用 `building:material` / `building:colour` / `roof:material` / `roof:colour` 覆盖默认材质。
- 根据类别选择屋顶、窗户、墙体深度、装饰和屋顶设备。
- 用 Minecraft 方块语法表达墙、窗、门、檐口、扶壁、玻璃幕墙、设备等细节。

本 PRD 将这些思想移植到现有 Godot mesh 生成管线。

## Terms

- **Arnis-style facade grammar**：参考 Arnis 的分类、材质、屋顶和立面规则，用 Godot mesh 而非 Minecraft 方块生成建筑细节。
- **建筑类别**：从 OSM 标签和高度推断出的用途/风格，如住宅、办公、酒店、仓库、工业、学校、医院、宗教、高层、历史等。
- **立面语法**：窗户、门、墙面深度、柱、横带、檐口、阳台、百叶、扶壁、玻璃幕墙等细节生成规则。
- **E2E 城市项目**：用 CLI 重新生成的 Godot 项目，并用 Godot headless 脚本验证主场景可运行和可漫游。

## Requirements

### REQ-0001-001: 细分建筑类别推断

- **Motivation**：OSM 里的 `building=*`、`amenity=*`、`shop=*`、`tourism=*` 等标签能提供用途线索，类别越准确，生成风格越可控。
- **Scope**：在现有 `BuildingCategory` 基础上细分至少以下类别：住宅/独栋住宅、商业/办公、酒店、工业、仓库/车库/棚屋、学校/医院、宗教、历史、高层/玻璃高层、温室、默认。
- **Non-goals**：不要求每个 OSM 标签都有独立类别；不做机器学习分类。
- **Acceptance**：单元测试覆盖每个新增或细分类别，输入固定标签集合时返回确定类别。

### REQ-0001-002: OSM 材质与颜色优先级

- **Motivation**：建筑“白膜感”的关键原因之一是材质过少、OSM 材质标签未充分参与决策。
- **Scope**：材质选择按优先级处理：显式 OSM 材质/颜色标签优先，其次类别预设，最后默认材质。Godot 材质枚举可扩展为有限调色板，避免为每栋楼生成无限材质文件。
- **Non-goals**：本轮不实现任意 RGB 动态材质资源生成；不解析所有颜色别名。
- **Acceptance**：单元测试证明 `building:material`、`building:colour`、`roof:material`、`roof:colour` 能改变墙/屋顶材质，未知值回退到类别默认值。

### REQ-0001-003: 扩展屋顶语法

- **Motivation**：平屋顶过多会让城市单调；Arnis 会根据 `roof:shape` 和建筑类别生成多种屋顶。
- **Scope**：支持至少 `flat`、`gabled`、`hipped`、`skillion`、`pyramidal` 五类屋顶；住宅缺少显式 `roof:shape` 时可自动采用坡屋顶；复杂或退化轮廓必须稳定回退。
- **Non-goals**：本轮不追求完全精确的曲面 dome/onion 几何；如实现只作为近似装饰。
- **Acceptance**：单元测试覆盖显式屋顶标签、住宅默认坡屋顶、复杂轮廓回退路径；生成 mesh 顶点数非零且无 panic。

### REQ-0001-004: 类别化立面细节

- **Motivation**：建筑是否有趣主要取决于立面层次，而不是单纯高度和体块。
- **Scope**：按类别生成不同细节组合：住宅窗台/百叶/阳台，商业柱廊/大窗，工业梁架，公共建筑横带，历史建筑檐口/角石，宗教扶壁，高层竖向鳍片/玻璃幕墙。
- **Non-goals**：不做室内；不做真实可进入楼层。
- **Acceptance**：测试以 mesh 数量、细节材质和特征标记证明不同类别产生不同立面细节，且细节数量随楼层/边长变化。

### REQ-0001-005: 屋顶和边缘装饰

- **Motivation**：屋顶设备、烟囱、女儿墙、天线等细节能显著降低“空盒子”观感。
- **Scope**：按类别和标签生成女儿墙、屋顶设备、烟囱、天线、医院停机坪或类似可辨识屋顶标记。
- **Non-goals**：不实现可交互设备；不做物理碰撞级别细节。
- **Acceptance**：测试覆盖高层/商业的设备、住宅烟囱、医院屋顶标记，验证对应 mesh 存在。

### REQ-0001-006: 上海外滩 E2E 验证

- **Motivation**：用户明确希望生成更有变化的上海外滩，而不是规整单调的西安样例。
- **Scope**：重新生成上海外滩附近 Godot 项目，主场景加载地面、道路、光照、player 和至少 20 个分场景/建筑分布，E2E 自动验证可运行。
- **Non-goals**：不保证每栋真实地标与现实完全一致；不导入外部 3D 模型。
- **Acceptance**：Godot headless import 和 E2E 脚本通过；生成目录记录在 README；E2E 输出包含道路、建筑和 player 相关断言。

### REQ-0001-007: 导航与交互元数据注入

- **Motivation**：应用最终目标带有导航和驾车性质，Godot 场景里的道路和建筑必须保留 OSM 名称与基础标签，未来才能做路名显示、建筑 POI、路线规划、驾驶 HUD 和交互查询。
- **Scope**：生成建筑与道路 mesh/节点时写入稳定元数据：`osm_id`、`osm_kind`、`name`、`official_name`/`alt_name`（存在时）、`building`、`highway`、`amenity`、`shop`、`tourism`、`addr:*`、高度/层数、道路宽度/等级等有限字段。Godot 实例化时通过 `set_meta` 挂到对应节点。
- **Non-goals**：本轮不实现路线规划算法、驾车物理、HUD、语音导航或道路拓扑图；只保证数据被带入 Godot。
- **Acceptance**：单元/集成测试证明 `mesh_data/*.json` 中包含建筑名和道路名元数据，Godot 加载脚本会把这些字段写入实例 meta；E2E 至少验证一个道路或建筑节点可读到 `osm_id` / `osm_kind` 元数据。

## Global Non-goals

- 不把 Godot 建筑改成 Minecraft voxel 逐块渲染。
- 不接入外部 3DMR、Wikidata、Google/Earth、Cesium 或商业模型源。
- 不做地标手工建模；本轮聚焦通用建筑语法。
- 不牺牲现有道路、地面、光照、player 和 4.6 项目兼容性。

## Verification Summary

本 PRD 的验收必须至少通过以下命令链：

```powershell
cargo test --target-dir E:\tmp\osm-godot-target element_processing::buildings -- --nocapture
cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
cargo test --target-dir E:\tmp\osm-godot-target
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v4-arnis-buildings --import
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v4-arnis-buildings --script E:\development\osm-godot\tools\godot_player_e2e.gd
```
