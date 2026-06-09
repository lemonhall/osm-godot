# v6-osm-vegetation: OSM 面状植被与多样树形

## Goal

利用 OSM 已抓取的面状绿地标签生成可见草地 patch、树木和灌木，并让单棵树生成至少三类稳定形态，改善上海城市里绿植稀少和形状单调的问题。

## PRD Trace

- REQ-0006-001：识别 OSM 面状绿地
- REQ-0006-002：绿地面生成可见地表 patch
- REQ-0006-003：面内确定性撒布树木和灌木
- REQ-0006-004：树木与灌木形态多样化
- REQ-0006-005：Streaming 与端到端验证
- REQ-0006-006：植被不得拖累 FPS

## Scope

做：
- 新增 vegetation processor，识别 closed way 上的 forest/wood/grass/park/garden/scrub 等 OSM 标签。
- 为 vegetation area 写入 `VegetationGround_*` ground patch mesh。
- 在 polygon 内确定性撒布 `VegetationTree_*`、`VegetationConifer_*`、`VegetationShrub_*` 实例，数量按类型限流。
- 重构现有 `natural=tree` 节点生成，支持 broadleaf、conifer、shrub 三类 profile 和稳定尺寸扰动。
- 新增 Godot E2E，验证样本工程 mesh_data 与运行时 chunk loader 都能看到植被输出。

不做：
- 不处理 multipolygon relation。
- 不联网补全树木或绿地。
- 不裁剪建筑、道路、水面重叠。
- 不引入外部 3D asset 或纹理。
- 不重新生成完整全上海工程作为本轮 DoD。
- 不用主观手动漫游替代自动化 FPS E2E。

## Acceptance

1. `cargo test --target-dir E:\tmp\osm-godot-target vegetation` 先红后绿。
2. `vegetation_area_classifies_osm_green_polygons` 证明目标 OSM 标签被识别，非绿地不误识别。
3. `vegetation_area_writes_ground_patch_mesh` 证明 closed green way 输出 `VegetationGround_*` mesh。
4. `vegetation_scatter_is_deterministic_and_capped` 证明撒布位置稳定，单面实例数量有上限，并且森林类样本生成 3 个以上实例。
5. `tree_generation_has_multiple_profiles` 证明至少三类植被 profile 可生成，并且不再只有单一锥形树。
6. `tools/godot_vegetation_e2e.gd` 在 v6 样本工程 exit 0，输出 vegetation mesh/metadata 统计。
7. 同一 E2E 必须让 player 自动移动 `>= 500.0m`，并断言 `avg_fps >= 55.0`、`min_fps >= 30.0`。
8. `cargo test --target-dir E:\tmp\osm-godot-target` 通过。
9. `git diff --check` 与中文乱码扫描通过。

## Files

- `docs/prd/PRD-0006-osm-vegetation.md`
- `docs/plan/v6-index.md`
- `docs/plan/v6-osm-vegetation.md`
- `src/data_processing.rs`
- `src/element_processing/mod.rs`
- `src/element_processing/trees.rs`
- `src/element_processing/vegetation.rs`
- `src/scene_writer/tres_writer.rs`
- `tools/godot_vegetation_e2e.gd`
- `README.md` / `AGENTS.md`（如输出说明需要同步）

## Steps

1. **Red classification**：新增 vegetation 分类测试，运行 `cargo test --target-dir E:\tmp\osm-godot-target vegetation`，预期因缺少 processor/分类函数失败。状态：done。
2. **Red output**：新增 scene 写入测试，断言 vegetation ground mesh 和具体植被实例存在，预期失败。状态：done。
3. **Red tree profiles**：新增树形 profile 测试，断言 broadleaf/conifer/shrub 三类 profile，预期失败。状态：done。
4. **Green processor**：新增 `vegetation.rs`，实现 OSM 标签分类、polygon ground patch、确定性撒布与数量上限。状态：done。
5. **Green tree profiles**：重构 `trees.rs`，保留 `natural=tree` 路径，同时引入多 profile mesh 和稳定尺寸扰动。状态：done。
6. **Green verify**：运行 `cargo test --target-dir E:\tmp\osm-godot-target vegetation`，预期通过。状态：done。
7. **E2E sample**：创建本地最小 OSM JSON 样本，生成 `E:\tmp\osm-godot-vegetation-v6-e2e`。状态：done。
8. **Godot E2E**：运行 `tools\godot_vegetation_e2e.gd`，断言 mesh_data/运行时节点包含 vegetation 输出，并让 player 自动移动约 500m，记录 `walk_distance`、`avg_fps`、`min_fps`。状态：done。
9. **Regression**：运行 `cargo test --target-dir E:\tmp\osm-godot-target`、`git diff --check`、乱码扫描。状态：done。
10. **Review/Ship**：回写 v6 evidence 和差异，必要时更新 README/AGENTS，commit + push。状态：done。

## Risks

- **大范围撒布过重**：使用间距、bbox 面积和每面实例上限控制输出量。
- **FPS 回退**：E2E 设置硬阈值 `avg_fps >= 55.0`、`min_fps >= 30.0`；若不达标，优先降低撒布密度或把细节收敛到更少 mesh。
- **polygon 点位采样偏外**：采样点必须通过 point-in-polygon 后才生成；失败点直接跳过。
- **材质不足**：优先复用 `TerrainGrass`、`TreeLeaves`、`TreeTrunk`，必要时增加 `ShrubLeaves` 这类轻量材质。
- **视觉仍不够密**：v6 先交付可计数、可回归的绿地和多形态植被；更精细的道路/建筑避让和近景资产进入后续版本。
