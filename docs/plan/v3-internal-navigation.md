# v3 Plan: 游戏内离线导航与路线指引

## Goal

把 v2 的本地城市索引升级为真正可路由、可交互的游戏内导航模式。完成后，外滩导航样例工程可以从出生点搜索“外滩/外滩源”，沿本地道路 graph 规划路线，并在 Godot 中显示 HUD、路线、箭头和本机语音提示降级。

## PRD Trace

- REQ-0003-001：生成本地可路由道路图
- REQ-0003-002：游戏内本地目的地搜索与选择
- REQ-0003-003：离线路径规划与道路吸附
- REQ-0003-004：游戏内路线线条、方向箭头和指引 HUD
- REQ-0003-005：本机语音提示与安全降级
- REQ-0003-006：外滩导航样例可生成、可运行、可验证

## Scope

本轮会修改或新增：

- `src/element_processing/highways.rs`
- `src/scene_writer/mod.rs`
- `src/scene_writer/navigation.rs`
- `tools/godot_navigation_e2e.gd`
- `README.md`
- `docs/prd/PRD-0003-internal-navigation.md`
- `docs/plan/v3-index.md`
- `docs/plan/v3-internal-navigation.md`

本轮不做：

- 不接入高德、Google、OSM、Overpass 或任何外部运行时导航服务。
- 不做车辆驾驶系统、车道级导航、实时交通、交通规则和红绿灯。
- 不生成新的整上海导航项目作为硬验收；本轮以外滩样例闭环，机制可用于整上海。
- 不把所有 chunk 静态加载回 `master.tscn`。

## Acceptance

- 生成项目根目录包含可解析的 `navigation_graph.json`，其中 `nodes.len() > 0`、`edges.len() > 0`。
- `navigation_graph.json` 的 edge 包含 `from`、`to`、`cost`、`osm_id`、`highway`、可选 `name`，且 `cost > 0`。
- `master.tscn` 挂载 `NavigationController`，并引用 `res://scripts/navigation_controller.gd`。
- `navigation_controller.gd` 只读取 `navigation_index.json` 与 `navigation_graph.json`，不包含 `HTTPRequest`、`HTTPClient`、`WebSocketPeer` 或外部 URL。
- Godot E2E 调用 `start_navigation_to_query("外滩")` 或 fallback `外滩源` 后，路线 waypoint 数大于 2，当前指令非空，HUD/overlay 节点存在。
- headless E2E 不因 TTS 不可用失败。
- README 记录外滩 v7 导航工程生成命令、输出路径和游戏内按键。

## Steps

1. 写失败测试（Red）
   - `scene_writer`：断言 `navigation_graph.json` 存在且 graph schema 包含 nodes/edges。
   - `scene_writer`：通过新增 `add_navigation_road` 测试连续道路生成双向边、cost 和 metadata。
   - `scene_writer`：断言 `master.tscn` 引用 `navigation_controller.gd` 并有 `NavigationController` 节点。
   - `scene_writer`：断言脚本包含 `start_navigation_to_query`、本地 graph/index 加载、A*、route overlay、HUD 和 TTS guard。
   - `scene_writer`：断言脚本不包含网络 API 字符串。

2. 跑到红
   - 命令：
     ```powershell
     cargo test --target-dir E:\tmp\osm-godot-target navigation -- --nocapture
     cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
     ```
   - 预期：新增测试失败，失败原因分别对应 graph 文件、controller 脚本、master 挂载或 API 缺失。

3. 实现到绿
   - 新增 `src/scene_writer/navigation.rs`，定义 graph node/edge/road 输入，负责坐标量化、节点去重、双向边生成和 JSON 序列化。
   - 在 `highways::generate_highway` 中把道路中心线登记到 `SceneWriter`。
   - 在 `save_all` 中写出 `navigation_graph.json` 和 `scripts/navigation_controller.gd`。
   - 改 `master.tscn`：新增 controller ext_resource、`NavigationController` 节点和必要 NodePath。
   - `navigation_controller.gd` 实现本地搜索、最近节点吸附、A*、路线 overlay、HUD、方向指令、本机 TTS 降级。

4. 跑到绿
   - 命令：
     ```powershell
     cargo test --target-dir E:\tmp\osm-godot-target navigation -- --nocapture
     cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
     cargo test --target-dir E:\tmp\osm-godot-target
     ```
   - 预期：全部 exit code 为 0。

5. 生成外滩导航样例
   - 命令：
     ```powershell
     $env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "31.2290,121.4820,31.2455,121.5100" --output-dir E:\tmp\osm-godot-shanghai-bund-v7-navigation --chunk-size 128 --stream-radius 2
     ```
   - 预期：生成成功，包含 `navigation_graph.json`、`navigation_index.json`、`scripts/navigation_controller.gd`。

6. Godot import + 导航 E2E
   - 命令：
     ```powershell
     & 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v7-navigation --import --quit
     & 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v7-navigation --script E:\development\osm-godot\tools\godot_navigation_e2e.gd
     ```
   - 预期：全部 exit code 为 0；E2E 日志包含 graph 节点/边、目的地、waypoint 数、距离和当前指令。

7. 文档回写与回顾
   - 更新 README：记录导航数据、按键、外滩 v7 命令、输出路径和离线边界。
   - 更新 `docs/plan/v3-index.md`：M1-M5 状态和证据。
   - 编码检查：`git diff --text -- README.md docs` + 乱码扫描。

## Risks

- **道路 graph 不连通**：OSM 道路中心线可能因分片、桥隧或数据缺口断开。缓解：量化节点并允许最近节点吸附；无路时明确显示 `route_not_found`。
- **大图 A* 性能**：整上海 graph 会明显大于外滩。缓解：本轮先用 A* 和本地邻接表；后续可加空间索引、分区 graph 或后台线程。
- **TTS 平台差异**：Godot/系统 TTS 在 headless 或不同系统不可用。缓解：所有 TTS 调用有 guard，文本 HUD 是必选降级。
- **UI 与 FPS 输入冲突**：导航面板需要鼠标输入，FPS 玩家会捕获鼠标。缓解：打开导航面板时释放鼠标，关闭后恢复捕获。
