# v1 Plan: Arnis-style Godot 建筑增强

## Goal

把 `refs/arnis` 中“由 OSM 标签推断建筑类型，再用素体语法组合材质、屋顶和立面细节”的机制迁移到本仓库现有 Godot mesh 生成管线，让上海外滩生成项目中的建筑明显摆脱同质盒子观感。

## PRD Trace

- REQ-0001-001：细分建筑类别推断
- REQ-0001-002：OSM 材质与颜色优先级
- REQ-0001-003：扩展屋顶语法
- REQ-0001-004：类别化立面细节
- REQ-0001-005：屋顶和边缘装饰
- REQ-0001-006：上海外滩 E2E 验证
- REQ-0001-007：导航与交互元数据注入

## Scope

本轮会修改：

- `src/element_processing/buildings.rs`
- `src/scene_writer/geometry.rs`
- `src/scene_writer/tres_writer.rs`
- `src/scene_writer/tscn_writer.rs`（仅当新增材质/mesh 写出需要）
- `src/scene_writer/mod.rs` 或 `chunk_grid.rs`（用于 mesh 元数据结构扩展）
- `tools/godot_player_e2e.gd`（仅当需要新增建筑多样性断言）
- `README.md`

本轮不做：

- 不重写成 voxel/block renderer。
- 不导入外部地标模型。
- 不做室内生成。
- 不做路线规划、驾车物理、HUD 或语音导航；本轮只把道路/建筑可用元数据写入 Godot。
- 不破坏现有 Godot 4.6、道路独立场景、地面、光照、player 漫游能力。

## Acceptance

- `cargo test --target-dir E:\tmp\osm-godot-target element_processing::buildings -- --nocapture` 通过，且包含新增分类、材质、屋顶、立面和屋顶装饰断言。
- `cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture` 通过。
- `mesh_data/*.json` 中道路和建筑元素可携带有限 OSM 元数据，Godot 加载脚本会写入节点 meta。
- `cargo test --target-dir E:\tmp\osm-godot-target` 通过。
- 重新生成 `E:\tmp\osm-godot-shanghai-bund-v4-arnis-buildings`，命令记录在 README。
- Godot 4.6 headless import 通过。
- `tools\godot_player_e2e.gd` 在新项目上通过，主场景可运行并包含 player、道路、地面、光照和建筑。

## Steps

1. 写失败测试（Red）
   - 在 `src/element_processing/buildings.rs` 添加分类测试：office、hotel、warehouse、hospital、religious、historic、glassy/highrise 等输入必须分流到预期类别。
   - 添加材质测试：`building:material`、`building:colour`、`roof:material`、`roof:colour` 必须影响墙/屋顶材质，未知值必须回退。
   - 添加屋顶测试：`flat/gabled/hipped/skillion/pyramidal` 产生非空 mesh，住宅无 `roof:shape` 时默认坡屋顶。
   - 添加立面测试：不同类别产生不同细节 mesh，且高层/宗教/历史/住宅有可辨识特征。
   - 添加屋顶装饰测试：高层设备、住宅烟囱、医院标记至少各有一类输出。
   - 添加导航元数据测试：带 `name` 的道路和带 `name` / `addr:*` 的建筑写入 JSON 后必须保留 `osm_id`、`osm_kind` 和名称字段；Godot loader 脚本必须包含把 `metadata` 字典写入 `set_meta` 的逻辑。

2. 跑到红
   - 命令：
     ```powershell
     cargo test --target-dir E:\tmp\osm-godot-target element_processing::buildings -- --nocapture
     ```
   - 预期：新增测试失败，原因是类别/材质/屋顶/细节能力尚未实现或不足。

3. 实现到绿
   - 扩展建筑类别和 `BuildingStyle`。
   - 增加有限材质调色板和 OSM 标签映射。
   - 增加屋顶类型解析和 mesh 生成。
   - 扩展 `make_building_detail_meshes`，让不同类别输出不同窗口、柱、横带、檐口、扶壁、鳍片、阳台、百叶等 mesh。
   - 扩展 SceneElement/mesh JSON，让建筑和道路携带有限 OSM 元数据；加载脚本实例化后批量写入 `instance.set_meta(...)`。
   - 保持输出资源命名稳定，避免破坏场景写出。

4. 跑到绿
   - 命令：
     ```powershell
     cargo test --target-dir E:\tmp\osm-godot-target element_processing::buildings -- --nocapture
     cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
     cargo test --target-dir E:\tmp\osm-godot-target
     ```
   - 预期：全部 exit code 为 0。

5. 生成上海外滩 v4
   - 命令：
     ```powershell
     $env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "31.2290,121.4820,31.2455,121.5100" --output-dir E:\tmp\osm-godot-shanghai-bund-v4-arnis-buildings --chunk-size 128
     ```
   - 预期：目录生成成功，包含 `project.godot`、`scenes/master.tscn`、建筑分场景、道路场景和材质资源。

6. Godot import + E2E
   - 命令：
     ```powershell
     & 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v4-arnis-buildings --import
     & 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v4-arnis-buildings --script E:\development\osm-godot\tools\godot_player_e2e.gd
     ```
   - 预期：两个命令 exit code 为 0；E2E 日志中可看到主场景、player、道路和建筑断言通过。

7. 文档回写与回顾
   - 更新 README：记录新生成命令、新项目地址、建筑语法说明。
   - 更新 `docs/plan/v1-index.md`：把 M1-M5 状态和证据写入追溯矩阵。
   - 如施工中发现范围变化，先写 ECN 再改计划。

## Risks

- **OSM 标签稀疏**：很多建筑缺少材质/屋顶标签。缓解：类别默认语法必须足够丰富，不能只依赖显式标签。
- **mesh 数量膨胀**：立面细节过多可能影响 Godot import 和运行。缓解：细节按边长、楼层和类别限制数量。
- **退化轮廓**：OSM 轮廓可能自交、太小或边长不足。缓解：屋顶和细节生成必须有非 panic 回退。
- **既有回归**：道路、地面、光照和 player 曾经反复修复。缓解：E2E 必须覆盖主场景可运行和 player 存在。

## Review Notes

本轮实现按 Red -> Green -> E2E 闭合：

- `element_processing::buildings`：8 passed，覆盖分类、材质、屋顶、立面和屋顶设备。
- `scene_writer`：28 passed，覆盖 mesh metadata 写出和 Godot loader 元数据挂载。
- 全量 `cargo test --target-dir E:\tmp\osm-godot-target`：66 passed, 1 ignored。
- 上海外滩 v4 工程已生成：`E:\tmp\osm-godot-shanghai-bund-v4-arnis-buildings\project.godot`。
- Godot 4.6 headless import exit 0，无迁移错误、脚本错误或 metadata key 错误。
- Godot E2E exit 0，验证道路、player 移动、鼠标视角/noclip、OSM 元数据可读。

完成信号只在最终收尾回复中输出；如仓库提交/推送未执行，不输出 `v_task_fully_done`。
