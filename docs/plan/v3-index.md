# v3: 游戏内离线导航与路线指引

## Vision

关联 PRD：[PRD-0003: 游戏内离线导航与路线指引](../prd/PRD-0003-internal-navigation.md)

本轮目标是在 Godot 城市里做出可用的“导航模式”：玩家按键打开导航 UI，搜索本地 index 里的目的地，选择后沿本地路网计算路线，并在 3D 世界和 HUD 上看到路线、箭头、剩余距离、下一步转向和本机语音提示。运行时不调用任何外部导航、寻址、路径规划或语音服务。

## Milestones

| Milestone | Scope | DoD | Verification | Status |
|---|---|---|---|---|
| M1 文档与验收冻结 | PRD-0003、v3 index、v3 计划 | 每条 REQ 都有计划、测试或 E2E；DoD 可二元判定 | `git diff --text -- docs` + 编码扫描 | done |
| M2 本地路网图生成 | `navigation_graph.json` 从道路中心线生成 | graph 包含节点、边、road metadata、cost；项目根目录落盘 | `cargo test ... navigation` + `scene_writer` 测试 | done |
| M3 运行时离线导航控制器 | 搜索、吸附、A*、路线状态 | `NavigationController` 能从 query 计算 route，不联网 | `cargo test ... scene_writer` + Godot E2E | done |
| M4 指引 HUD、路线线条与语音降级 | UI、overlay、箭头、指令、TTS guard | HUD/overlay 节点存在，headless 不因 TTS 失败 | Godot E2E | done |
| M5 外滩导航样例与文档 | 生成 v7 外滩项目并回写 README | Godot import/E2E exit 0；README 有命令和路径 | 外滩 v7 导航工程 + README | done |

## Plan Index

- [v3-internal-navigation.md](v3-internal-navigation.md)

## Traceability Matrix

| Req ID | PRD | v3 Plan | Unit/Integration Tests | E2E | Evidence | Status |
|---|---|---|---|---|---|---|
| REQ-0003-001 | PRD-0003 §REQ-0003-001 | v3-internal-navigation §Step 1-4 | `navigation_graph_is_written_from_highway_centerlines` | 外滩导航 E2E graph load | v7 graph `7711` nodes / `29986` edges | done |
| REQ-0003-002 | PRD-0003 §REQ-0003-002 | v3-internal-navigation §Step 1-4 | `navigation_controller_uses_local_graph_and_has_no_network_api` | `start_navigation_to_query("外滩")` | E2E status `routing` | done |
| REQ-0003-003 | PRD-0003 §REQ-0003-003 | v3-internal-navigation §Step 1-4 | graph snap/A* 脚本内容测试 | route waypoint + distance | E2E `waypoint_count=74`，`total_distance=1148.3756` | done |
| REQ-0003-004 | PRD-0003 §REQ-0003-004 | v3-internal-navigation §Step 1-4 | `master_scene_mounts_navigation_controller` + 导航面板 UI 测试 | overlay/HUD 节点断言 + 面板焦点 E2E | E2E `panel_centered=true`、`controls_disabled=true`、`route_arrow_exists=true` | done |
| REQ-0003-005 | PRD-0003 §REQ-0003-005 | v3-internal-navigation §Step 1-4 | TTS guard 内容测试 | headless E2E 不报错 | E2E headless exit 0，脚本启用 `DisplayServer.tts_speak` 并优先中文 voice | done |
| REQ-0003-006 | PRD-0003 §REQ-0003-006 | v3-internal-navigation §Step 5-7 | 全量 cargo test | 外滩 v7 Godot import/E2E | `E:\tmp\osm-godot-shanghai-bund-v7-navigation` import/E2E exit 0 | done |

## ECN Index

当前无 ECN。若施工中发现必须接入外部寻路、外部 geocoding 或云 TTS 才能完成需求，必须停止并写 ECN；不得口头改变“全 Godot 内部完成”的边界。

## DoD Hardness Gate

- [x] 每条需求均有二元验收：文件存在、JSON 可解析、节点/边数量、route waypoint 数、HUD 节点存在、E2E exit code。
- [x] 每条需求均绑定验证命令或脚本。
- [x] 反作弊条款：不得只复用 `navigation_index.json` 伪装成导航；必须生成 `navigation_graph.json`，且 route 由 graph 节点/边计算。
- [x] 反作弊条款：不得调用高德、Google、OSM、Overpass 或任何外部运行时 API；Godot 导航脚本不得包含 HTTP 请求类或网络路径。
- [x] 反作弊条款：不得只画一条出生点到目的地的直线；E2E 必须断言 route waypoint 数大于 2，路线来自 graph。
- [x] Scope 明确排除车辆物理、实时交通、车道导航、在线搜索和外部 TTS。

## Difference List

已关闭差异：

- v2 只有 `navigation_index.json`，只能搜对象；v3 新增 `navigation_graph.json`，从道路中心线生成节点、边、道路名、道路等级和 cost。
- Godot 主场景新增 `NavigationController`，运行时只读取本地 index/graph，脚本内容测试禁止 `HTTPRequest`、`HTTPClient`、`WebSocketPeer` 和外部 URL。
- 导航目的地吸附不再只选最近节点；若最近节点落在孤立小分量，会尝试多个近邻路网节点并选择可连通路线。
- 生成期为几何上接近但未共享 OSM 节点的道路加入近邻连接边，避免交叉口数据轻微断裂导致 `route_not_found`。
- 外滩 v7 导航样例已生成并通过 Godot 4.6 headless import/E2E。
- 导航面板交互修正：面板居中，按钮为“取消 / 开始导航”，打开时释放鼠标并暂停 FPS 控制，关闭后恢复。
- 指引可视化和播报修正：路线箭头改为高亮绿色 unshaded/emission 材质，提示文案改为中文，并通过 Godot 本机 TTS 播报。
- 开始导航稳定性修正：开始按钮使用稳定节点名和防重入状态，语音播报延后一帧触发，避免真实点击时按钮事件与系统 TTS 同栈执行。

验证证据：

- `cargo test --target-dir E:\tmp\osm-godot-target`：85 passed，1 ignored。
- 外滩 v7 生成命令 exit 0，输出 `E:\tmp\osm-godot-shanghai-bund-v7-navigation`。
- Godot import：`--headless --path E:\tmp\osm-godot-shanghai-bund-v7-navigation --import --quit` exit 0，无 `SCRIPT ERROR`。
- 导航 E2E：`tools\godot_navigation_e2e.gd` exit 0，日志包含 `panel_centered=true`、`controls_disabled=true`、`mouse_visible=true`、`graph_nodes=7711`、`graph_edges=29986`、`waypoint_count=74`、`total_distance=1148.37562561035`、`status=routing`。
