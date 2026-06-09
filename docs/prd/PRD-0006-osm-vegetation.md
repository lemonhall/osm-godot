# PRD-0006: OSM 植被与绿地生成

## Vision

玩家在上海等大范围 Godot 城市中漫游时，公园、林地、草地和绿化带不再只是空地或单调地表。生成器应利用 OSM 已抓取的面状绿地标签，生成可见的草地 patch、树木和灌木；单棵树也应有多种稳定形态，避免满城只有同一种锥形树。

## Context

现有 Overpass 查询已经拉取 `landuse`、`natural`、`leisure` 等元素，但 `data_processing.rs` 只把 `natural=tree` 节点交给 `trees.rs`。`landuse=forest`、`natural=wood`、`leisure=park`、`landuse=grass`、`natural=scrub` 等面状绿地没有进入场景输出。现有树模型由圆柱树干和锥形树冠组成，形态单一，且无法表达灌木、草地或林地密度。

v6 将新增 OSM vegetation 处理链：识别面状绿地，输出地表 patch，并在面内按确定性规则撒布树木/灌木。生成必须保持 streaming chunk 模型，不能把全图植被静态写入 master。

## Requirements

### REQ-0006-001: 识别 OSM 面状绿地

- **Motivation**：城市里的绿量主要来自公园、林地、草坪和绿化带，不应只依赖稀疏的 `natural=tree` 节点。
- **Scope**：识别 closed way 上的 `landuse=forest/grass/meadow/recreation_ground/village_green`、`natural=wood/scrub/grassland/heath`、`leisure=park/garden`。
- **Non-goals**：不解析 multipolygon relation；不联网补数据；不把非闭合线性绿道当作面处理。
- **Acceptance**：单元测试证明上述标签会被分类为 vegetation area，且非绿地 closed way 不生成植被。

### REQ-0006-002: 绿地面生成可见地表 patch

- **Motivation**：即使不撒树，公园和草坪也应在地表上可见，避免城市块中出现空白荒地。
- **Scope**：面状绿地生成低矮 polygon mesh，材质按 woodland、grass、scrub 三类选择。
- **Non-goals**：不做复杂地形贴合；不裁剪建筑/道路重叠；不生成透明贴图草。
- **Acceptance**：测试证明 vegetation area 会向 scene 写入 `VegetationGround_*` mesh，材质不是建筑/道路材质。

### REQ-0006-003: 面内确定性撒布树木和灌木

- **Motivation**：OSM 面只能说明范围，玩家需要看到具体的树和绿植对象。
- **Scope**：在 vegetation polygon bbox 内按固定网格步长采样，使用 OSM id 派生的确定性扰动，点在 polygon 内才生成实例；forest/wood 生成更多树，park/garden 生成中等树和灌木，grass/meadow 生成少量灌木。
- **Non-goals**：不做道路/建筑避让布尔运算；不追求真实树种；不生成无限密度对象。
- **Acceptance**：测试证明同一 OSM id 和 polygon 每次生成数量与位置一致；每个面生成数量有上限；至少一种绿地面能生成 3 个以上植被实例。

### REQ-0006-004: 树木与灌木形态多样化

- **Motivation**：现有树形太单调，视觉上缺乏趣味性。
- **Scope**：至少支持 broadleaf、conifer、shrub 三种形态；实例高度、冠幅和段数由确定性种子变化；树干和叶冠使用不同材质或不同 mesh 输出。
- **Non-goals**：不引入外部 3D asset；不做风吹动画；不做近景高模树。
- **Acceptance**：测试证明生成脚本/mesh 数据中存在至少三类植被命名或 metadata；单棵 `natural=tree` 不再只走单一锥形轮廓。

### REQ-0006-005: Streaming 与端到端验证

- **Motivation**：全上海项目必须继续按 chunk 加载，植被不能破坏运行时 streaming。
- **Scope**：植被输出进入现有 chunk mesh/mesh_data 流程；新增 Godot E2E 检查最小样本工程加载后存在 vegetation ground 和多类植被 mesh。
- **Non-goals**：不要求本轮重新生成全上海完整项目；不做人工视觉截图验收；性能阈值由 REQ-0006-006 单独约束。
- **Acceptance**：`tools/godot_vegetation_e2e.gd` 在 v6 样本工程 exit 0；全量 `cargo test` 通过；`git diff --check` 通过。

### REQ-0006-006: 植被不得拖累 FPS

- **Motivation**：绿植是视觉增强，不能让城市漫游变卡，尤其不能破坏上海 streaming 项目的可玩性。
- **Scope**：植被撒布必须有每面实例上限和全局密度控制；Godot E2E 必须让 player 在 v6 样本工程里自动移动约 500m，采样平均 FPS、最低 FPS 和加载后 vegetation 节点数量。
- **Non-goals**：不以一次 headless FPS 数字代表所有显卡；不要求本轮完成 LOD/billboard 系统；不把手动主观流畅度当验收。
- **Acceptance**：`tools/godot_vegetation_e2e.gd` 输出 `walk_distance >= 500.0`、`avg_fps >= 55.0`、`min_fps >= 30.0`，且 vegetation mesh/实例数量大于 0；不满足任一条件时 exit 1。

## Verification Commands

```powershell
cargo test --target-dir E:\tmp\osm-godot-target vegetation
cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
cargo test --target-dir E:\tmp\osm-godot-target
cargo run --target-dir E:\tmp\osm-godot-target -- --file E:\tmp\osm-godot-vegetation-osm.json --bbox "31.2290,121.4820,31.2455,121.5100" --output-dir E:\tmp\osm-godot-vegetation-v6-e2e --chunk-size 128 --stream-radius 1
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-vegetation-v6-e2e --script E:\development\osm-godot\tools\godot_vegetation_e2e.gd
```
