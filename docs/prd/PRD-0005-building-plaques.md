# PRD-0005: 建筑中文门匾

## Vision

玩家在流式加载的 Godot 城市里靠近建筑时，可以看到带正式中文名称的建筑门匾。普通楼宇在门旁显示白底黑字竖牌；便利店、商店、餐饮等店铺类建筑优先在门上方显示横匾。没有正式中文名称的建筑不挂牌，避免满城假名或英文噪音。

## Context

当前生成器已经把道路和建筑的 OSM metadata 写入 `mesh_data/*.json`，Godot chunk loader 会为带 metadata 的元素创建 marker。现有 metadata 只保留 `name`、`official_name`、`alt_name`、`old_name` 等通用字段，尚未保留 `name:zh` / `official_name:zh` 等中文命名字段；chunk loader 也没有基于 metadata 生成世界空间文字牌匾。

建筑 mesh 没有显式“门”的语义点，但生成期已有 building wall mesh 和建筑 footprint。v5 将“门匾位置”定义为：从建筑 wall mesh 外轮廓估算一条最长门面边，牌匾挂在该边外侧附近。店铺类建筑生成横匾，普通建筑生成竖匾。

## Requirements

### REQ-0005-001: 只给正式中文名称建筑挂牌

- **Motivation**：牌匾必须像真实中文城市环境，不应给无名楼、英文名或类别名生成假牌。
- **Scope**：保留 `name:zh`、`official_name:zh`、`alt_name:zh`、`brand:zh`、`operator:zh` 等中文命名字段；牌匾文本优先级为 `official_name:zh`、`name:zh`、纯中文 `official_name`、纯中文 `name`、`brand:zh`、`operator:zh`。
- **Non-goals**：不翻译英文名；不从 `building` / `amenity` 类别合成中文名；不使用外部 API 查询名称。
- **Acceptance**：单元测试证明中文命名字段写入 metadata；loader 脚本存在中文名筛选函数，英文名和空名不会生成牌匾。

### REQ-0005-002: 每个建筑最多一个牌匾

- **Motivation**：一个建筑会输出 wall、roof、window、door、trim 等多个带相同 metadata 的 mesh，不能重复挂牌。
- **Scope**：Godot chunk loader 以 `osm_id` 对建筑牌匾去重；只处理 `osm_kind=building` 或带 `building` 字段的 metadata。
- **Non-goals**：不在同一建筑多个入口生成多个牌匾；不做楼内商铺多招牌。
- **Acceptance**：脚本内容测试证明存在已挂牌建筑集合，且牌匾创建路径会记录 `osm_id`。

### REQ-0005-003: 普通建筑竖牌，店铺类横匾

- **Motivation**：机关/学院/楼宇常见门旁竖牌，便利店/商铺常见门上横匾。
- **Scope**：普通建筑生成白底黑字竖牌，文本按单字换行；`shop`、`amenity=restaurant/cafe/fast_food/pharmacy/bank`、`tourism=hotel`、`building=retail/commercial/supermarket` 等店铺/服务类生成白底黑字横匾。
- **Non-goals**：不做复杂字体嵌入；不保证真实门洞位置；不做发光招牌。
- **Acceptance**：脚本内容测试证明存在横/竖两种牌匾构建路径、白色背景、黑色 Label3D 文本和按店铺 metadata 分类的函数。

### REQ-0005-004: 牌匾随 chunk streaming 生命周期加载/卸载

- **Motivation**：全上海项目必须保持 streaming 模型，不能把所有牌匾静态挂到 master。
- **Scope**：牌匾由 `chunk_mesh_loader.gd` 在 chunk 加载时创建为 chunk 子节点，随 chunk unload 被释放。
- **Non-goals**：不修改 `master.tscn` 为全图静态引用；不生成独立全图牌匾索引。
- **Acceptance**：单元/脚本测试证明牌匾逻辑在 chunk loader 中，`master.tscn` 不需要静态牌匾节点。

### REQ-0005-005: 端到端验证

- **Motivation**：需要证明 Godot 运行时能从 metadata 创建真实 3D 牌匾，而不是只写了模板字符串。
- **Scope**：新增 Godot E2E 脚本，构造中文名建筑 marker/mesh 数据，触发 chunk loader 后断言生成 `BuildingPlaque` 节点、文本为中文、英文名建筑不生成。
- **Non-goals**：不要求在全上海重新生成前完成视觉人工审查。
- **Acceptance**：`tools/godot_building_plaques_e2e.gd` 在测试工程中 exit 0，并输出生成数量和过滤结果。

## Verification Commands

```powershell
cargo test --target-dir E:\tmp\osm-godot-target building_plaque
cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
cargo test --target-dir E:\tmp\osm-godot-target
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\<plaque-project> --script E:\development\osm-godot\tools\godot_building_plaques_e2e.gd
```
