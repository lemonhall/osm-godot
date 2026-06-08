# PRD-0003: 游戏内离线导航与路线指引

## Vision

用户在生成出的 Godot 城市里，可以像使用“地图导航模式”一样打开游戏内导航面板，搜索本地工程里已有的道路或建筑目的地，选择后获得路线、方向箭头、下一步转向提示和本机语音播报。所有寻址、路径规划和指引都在 Godot 工程内部完成，只读取生成期落盘的本地 JSON 数据，不调用高德、OSM、Google 或任何外部导航、搜索、TTS 服务。

## Background

v2 已经把大范围城市拆成 streaming chunk，并生成 `navigation_index.json`，其中保留道路和建筑的名称、类别、OSM id、bbox、center 与 chunk。这个索引能回答“有哪些对象可以搜”，但不能回答“怎么沿道路走过去”。下一步需要在生成期把道路中心线转换成轻量可路由图，并在 Godot 运行时提供可交互的离线导航体验。

用户已经明确：想要的是类似高德地图的导航体验，而不是接入高德、OSM 或其他在线导航能力。因此本 PRD 把“体验形态”和“数据来源”分开：UI、箭头、提示可以借鉴手机导航；寻路、寻址、播报必须只依赖生成出的 Godot 项目文件和本机能力。

## Terms

- **Navigation graph**：生成期从道路中心线输出的轻量路网 JSON，包含节点、边、道路名、道路等级和成本。
- **Destination index**：现有 `navigation_index.json`，用于本地搜索道路、建筑、POI 和名称。
- **Offline routing**：Godot 运行时读取本地 graph/index 后，通过最近道路点吸附和 A* 寻路计算路线。
- **Guidance HUD**：游戏内导航 UI，展示目的地、路线距离、下一步指令、方向箭头和错误状态。
- **Voice prompt**：优先调用 Godot/系统本机 TTS 播报提示；不可用时只显示屏幕文字，不请求外部语音服务。
- **Route overlay**：绘制在世界中的路线线条和前进箭头，用于玩家漫游时识别路线。

## Requirements

### REQ-0003-001: 生成本地可路由道路图

- **Motivation**：只靠 chunk mesh 和 `navigation_index.json` 无法进行沿路寻路。
- **Scope**：生成期从 OSM 道路 `ProcessedWay.nodes` 中提取中心线，按 Godot 世界坐标输出 `navigation_graph.json`，包含节点、边、道路名、`highway`、`osm_id`、边长成本和图版本。
- **Non-goals**：不做实时交通、车道级导航、单行/禁行规则、跨城市在线补图。
- **Acceptance**：单元测试证明两段相交或连续道路会生成节点、双向边、道路 metadata 和非零 cost；生成项目根目录存在 `navigation_graph.json` 且 JSON 可解析。

### REQ-0003-002: 游戏内本地目的地搜索与选择

- **Motivation**：用户需要在游戏里输入“外滩”等目的地，而不是手动记坐标。
- **Scope**：新增 Godot `navigation_controller.gd`，读取 `navigation_index.json`，支持按名称、官方名、别名、道路名和建筑/POI 类型进行本地搜索，UI 中可选择目的地并开始导航。
- **Non-goals**：不联网搜索，不调用高德/OSM geocoding，不做拼音/模糊语义纠错。
- **Acceptance**：Godot E2E 调用 `start_navigation_to_query("外滩")` 或 `start_navigation_to_query("外滩源")` 能命中本地 index 条目；查询不存在时返回失败并显示错误，不崩溃。

### REQ-0003-003: 离线路径规划与道路吸附

- **Motivation**：玩家当前位置和目的地中心点通常不在路网节点上，需要吸附到最近道路节点后计算路线。
- **Scope**：运行时读取 `navigation_graph.json`，建立邻接表；将玩家位置和目的地 center 吸附到最近图节点；使用 A* 计算路线，返回 Godot 世界坐标 waypoint 列表和总距离。
- **Non-goals**：不承诺 OSM 原始数据断裂时一定有路；断路时必须给出明确失败状态。
- **Acceptance**：Godot E2E 证明从出生点到外滩目标得到至少 2 个 waypoint、总距离大于 0、当前指令非空；无路时返回 `route_not_found` 状态。

### REQ-0003-004: 游戏内路线线条、方向箭头和指引 HUD

- **Motivation**：玩家需要在 3D 城市里看到“往哪走”和“下一步干什么”。
- **Scope**：在 `master.tscn` 中挂载 `NavigationController` 和 `CanvasLayer` UI；当前阶段路线用连续绿色世界线条可视化，并在终点显示绿色到达圈；玩家进入到达圈后清理路线带、到达圈和导航 HUD 文本。HUD 显示目的地、路线状态、搜索框和候选列表；快捷键打开/关闭面板。路口 marker 与语音播报暂缓。
- **Non-goals**：不做完整手机地图 UI，不做小地图、不做车辆仪表盘。
- **Acceptance**：Godot E2E 证明 `NavigationController`、HUD 节点、route overlay 节点存在；开始导航后路线节点包含可见 mesh，HUD 指令文字非空。

### REQ-0003-005: 本机语音提示与安全降级

- **Motivation**：导航模式需要“左转/右转/直行”等语音感知，但不能接外部 TTS。
- **Scope**：指令变化或距离阈值触发时，优先使用 Godot/系统本机 TTS 接口播报；若接口不可用或 headless 运行，则只更新 HUD 文本并记录状态。
- **Non-goals**：不调用云 TTS，不下载语音包，不要求所有平台都有声音。
- **Acceptance**：Godot E2E 在 headless 下不会因 TTS 不可用报错；脚本包含本机 TTS 调用保护和 `last_spoken_instruction` 节流。

### REQ-0003-006: 外滩导航样例可生成、可运行、可验证

- **Motivation**：用户希望能从出生点走向外滩，并用它验证导航闭环。
- **Scope**：生成新的上海外滩导航工程，保留 v2 streaming、player、道路、地面、光照、建筑多样性；新增导航 graph/index/UI/overlay。
- **Non-goals**：不生成整个上海的新导航包作为本轮硬验收；大上海导航可复用同机制，但本轮 E2E 以外滩范围闭环。
- **Acceptance**：README 记录生成命令和输出路径；Godot 4.6 headless import exit 0；`tools/godot_navigation_e2e.gd` exit 0，日志包含 graph 节点/边数量、目的地名称、路线 waypoint 数、总距离和当前指令。

## Global Non-goals

- 不调用高德、Google、OSM、Overpass 或其他在线服务做运行时导航、寻址、路径规划或语音播报。
- 不实现真实车辆物理、车道级导航、实时交通、限行规则、红绿灯策略。
- 不要求 OSM 断裂道路自动补全；只在生成数据可连通时给出路线。
- 不把所有 chunk 静态加载回主场景。

## Verification Summary

```powershell
cargo test --target-dir E:\tmp\osm-godot-target navigation -- --nocapture
cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
cargo test --target-dir E:\tmp\osm-godot-target
cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "31.2290,121.4820,31.2455,121.5100" --output-dir E:\tmp\osm-godot-shanghai-bund-v7-navigation --chunk-size 128 --stream-radius 2
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v7-navigation --import --quit
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\osm-godot-shanghai-bund-v7-navigation --script E:\development\osm-godot\tools\godot_navigation_e2e.gd
```
