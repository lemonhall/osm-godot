# CLAUDE.md

本文件是 osm-godot 项目的机读协作规约摘要。完整、优先维护的规则见 [AGENTS.md](AGENTS.md)；如果两者冲突，以 `AGENTS.md` 和用户显式指令为准，并同步更新本文件。

## 项目概览

osm-godot 是一个 Rust CLI 工具，用 OpenStreetMap 数据、可选卫星高程和 ESA WorldCover 地表分类生成 Godot 4.6 3D 城市场景工程。当前输出是 runtime streaming world package：Godot 主场景只挂环境、玩家、streamer 和导航控制器，chunk 几何在 `mesh_data/*.json` 中按需加载。

参考项目是 `refs/arnis/`。该目录是只读参考实现，负责启发 OSM/高程/地表分类输入管线；本项目的 Godot 输出层位于 `src/scene_writer/`。

## 快速命令

在 `E:\development\osm-godot` 下运行，默认使用 PowerShell：

```powershell
cargo check --target-dir E:\tmp\osm-godot-target
cargo test --target-dir E:\tmp\osm-godot-target
cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture
git diff --check
```

生成小区域：

```powershell
cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "48.1360,11.5770,48.1363,11.5775" --output-dir E:\tmp\osm-godot-munich-small
```

通过本机代理抓取 OSM：

```powershell
$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "34.210594,108.947432,34.226406,108.969568" --output-dir E:\tmp\osm-godot-xian-yanta-style --chunk-size 128
```

整上海范围使用分片抓取和本地 cache：

```powershell
$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --release --target-dir E:\tmp\osm-godot-target-release -- --bbox "30.67,120.85,31.88,122.12" --output-dir E:\tmp\osm-godot-shanghai-city-navigation-c512 --chunk-size 512 --stream-radius 1 --tiled-fetch --fetch-tile-degrees 0.25 --tile-cache-dir E:\tmp\osm-godot-cache\shanghai
```

## 架构

```text
retrieve_data.rs -> osm_parser.rs -> data_processing.rs
                                      |
          +---------------------------+---------------------------+
          v                           v                           v
 element_processing/buildings.rs  highways.rs              water/trees/rail
          +---------------------------+---------------------------+
                                      v
                              scene_writer/mod.rs
          +---------------------------+---------------------------+
          v                           v                           v
     chunk_grid.rs              tscn_writer.rs              tres_writer.rs
          |                           |                           |
          +---------------------------+---------------------------+
                                      v
 project.godot + scenes/ + materials/ + scripts/ + mesh_data/
 world_manifest.json + navigation_index.json + navigation_graph.json
```

关键模块：

- `src/args.rs`：CLI 参数；README 参数表必须与它保持一致。
- `src/data_processing.rs`：把 OSM way/node 分派到建筑、道路、水域、铁路、树木等处理器。
- `src/element_processing/buildings.rs`：建筑分类、材质、屋顶、立面细节。
- `src/element_processing/highways.rs`：道路 mesh 和导航图中心线输入。
- `src/scene_writer/mod.rs`：写 master scene、runtime GDScript、manifest、导航数据和资源。
- `src/scene_writer/navigation.rs`：本地路网图构建。
- `tools/*.gd`：Godot headless E2E 脚本。

## 当前输出模型

- `scenes/master.tscn`：WorldEnvironment、Sun、Clouds、WorldFloor、Player、WorldStreamer、NavigationController。
- `scenes/Chunk_X_Z.tscn`：轻量 chunk loader。
- `mesh_data/Chunk_X_Z.json`：该 chunk 的 mesh、材质、transform、OSM metadata。
- `world_manifest.json`：streaming chunk 清单。
- `navigation_index.json`：道路/建筑搜索索引。
- `navigation_graph.json`：本地 A* 路由图。
- `scripts/chunk_mesh_loader.gd`：线程读取 JSON，按材质合批 `ArrayMesh`，保留 metadata marker。
- `scripts/world_streamer.gd`：按玩家所在 chunk 加载/卸载附近 chunk。
- `scripts/navigation_controller.gd`：本地导航、绿色路线带、G 自动巡航、F 键建筑查看。
- `scripts/fps_player.gd`：FPS 移动、noclip、自动移动和控制暂停 API。

## 安全边界

- 不要修改 `refs/arnis/`。
- 不要把 build target 或生成 Godot 工程写进仓库；使用 `E:\tmp\...` 和 `--target-dir E:\tmp\osm-godot-target`。
- 不要执行递归删除或批量清理，除非用户明确确认具体路径。
- 不要让导航逻辑依赖网络服务；Godot 运行时导航必须只读本地 JSON。
- 不要提交 secrets、`.env`、私钥、APNs 凭据或代理凭据。

## 验证

常用 Rust 验证：

```powershell
cargo test --target-dir E:\tmp\osm-godot-target
cargo test --target-dir E:\tmp\osm-godot-target navigation -- --nocapture
cargo test --target-dir E:\tmp\osm-godot-target building_inspection
```

常用 Godot 4.6 headless 路径：

```powershell
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\<project> --import --quit
```

E2E 脚本：

- `tools\godot_player_e2e.gd`
- `tools\godot_streaming_e2e.gd`
- `tools\godot_streaming_perf_probe.gd`
- `tools\godot_navigation_e2e.gd`
- `tools\godot_navigation_autorun_e2e.gd`
- `tools\godot_building_inspection_e2e.gd`

## 当前状态与边界

- 已实现：Arnis-style 建筑、runtime chunk streaming、道路/建筑 OSM metadata、本地导航图、导航 UI、G 自动巡航、F 键建筑信息查看。
- 已知边界：`metadata.json` 中地理边界仍是 `save_all()` 写出的占位值；真实定位主要依赖生成命令中的 bbox 与 chunk/scale 元数据。
- 大范围上海生成是长任务，不要用短超时误判失败。超时后先检查进程和输出目录。
