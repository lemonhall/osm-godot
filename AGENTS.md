# Agent Notes (osm-godot)

本文件是 osm-godot 的 AI 协作规约。目标是让 agent 在 Windows/PowerShell 环境里做出可运行、可验证、不会误伤参考工程和生成产物的改动。

## Project Overview

osm-godot 是一个 Rust CLI 工具，用 OpenStreetMap 数据、可选卫星高程和 ESA WorldCover 地表分类生成 Godot 4.6 3D 城市场景工程。输出模型已经从静态全量场景演进为 runtime streaming world package：Godot 主场景只挂环境、玩家、streamer 和导航控制器，chunk 几何在 `mesh_data/*.json` 中按需加载。

参考实现位于 `refs/arnis/`。它把 OSM 转为 Minecraft 世界；本项目复用输入/解析思路，但输出为 Godot `project.godot`、`.tscn`、`.tres`、GDScript、JSON mesh 数据和本地导航数据。

## Quick Commands

在仓库根目录 `E:\development\osm-godot` 下运行。默认 shell 是 PowerShell；连续命令用 `;`，不要写 bash 风格的 `&&`。

- Check: `cargo check --target-dir E:\tmp\osm-godot-target`
- Test full: `cargo test --target-dir E:\tmp\osm-godot-target`
- Test scene writer: `cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture`
- Test navigation: `cargo test --target-dir E:\tmp\osm-godot-target navigation -- --nocapture`
- Test building inspection: `cargo test --target-dir E:\tmp\osm-godot-target building_inspection`
- Diff check: `git diff --check`
- Generate small Xi'an sample from cached OSM:
  `cargo run --target-dir E:\tmp\osm-godot-target -- --file E:\tmp\osm-godot-xian-yanta-style-osm.json --bbox "34.2160,108.9550,34.2210,108.9620" --output-dir E:\tmp\osm-godot-xian-yanta-style-vN --chunk-size 128`
- Generate live OSM data through local proxy:
  `$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "34.210594,108.947432,34.226406,108.969568" --output-dir E:\tmp\osm-godot-xian-yanta-style-vN --chunk-size 128`
- Generate full Shanghai streaming/navigation project:
  `$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --release --target-dir E:\tmp\osm-godot-target-release -- --bbox "30.67,120.85,31.88,122.12" --output-dir E:\tmp\osm-godot-shanghai-city-vN-navigation-c512 --chunk-size 512 --stream-radius 1 --tiled-fetch --fetch-tile-degrees 0.25 --tile-cache-dir E:\tmp\osm-godot-cache\shanghai`

Prefer `--target-dir E:\tmp\osm-godot-target` so local build artifacts do not churn inside the repo. Generated Godot projects should also go under `E:\tmp\...` unless the user explicitly asks otherwise.

Full Shanghai generation is a long-running job. Do not use short timeouts such as 30 or 90 minutes and then assume failure. Use a several-hour timeout when available; if the command times out, first check for live `cargo` / `osm-godot` processes and the output directory before retrying or changing strategy.

## Architecture Overview

```text
OSM Overpass / local JSON
        |
        v
retrieve_data.rs -> osm_parser.rs -> Vec<ProcessedElement>
                                  |
                  optional terrain/land-cover ground grid
                                  |
                                  v
                         data_processing.rs
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
 output project: project.godot, scenes/, materials/, scripts/, mesh_data/,
 world_manifest.json, navigation_index.json, navigation_graph.json, assets/
```

Important modules:

- `src/main.rs`: CLI entry and pipeline orchestration.
- `src/args.rs`: Godot-specific CLI arguments. Keep README parameter docs aligned with this file.
- `src/retrieve_data.rs`: Overpass and tiled Overpass retrieval, including cache/save options.
- `src/osm_parser.rs`: OSM JSON to `ProcessedElement`.
- `src/data_processing.rs`: dispatch loop from parsed OSM elements into element processors.
- `src/element_processing/`: OSM element to scene mesh conversion.
- `src/element_processing/buildings.rs`: Arnis-style building classification, materials, roofs, facade details.
- `src/element_processing/highways.rs`: road surfaces and navigation graph centerline input.
- `src/ground_generation.rs`: terrain mesh generation.
- `src/scene_writer/mod.rs`: top-level Godot output orchestration, master scene, runtime scripts, manifests, navigation data.
- `src/scene_writer/chunk_grid.rs`: chunk partitioning and scene element storage.
- `src/scene_writer/tscn_writer.rs`: lightweight chunk loader scenes and `mesh_data/*.json`.
- `src/scene_writer/tres_writer.rs`: Godot material `.tres` writing.
- `src/scene_writer/project_writer.rs`: `project.godot`, default environment and `metadata.json`.
- `src/scene_writer/navigation.rs`: lightweight offline navigation graph generation.
- `tools/*.gd`: Godot 4.6 headless E2E probes for player, streaming, navigation, autorun, building inspection.
- `docs/prd/` and `docs/plan/`: versioned PRD/plan/evidence chain for v1-v4.

Current output model:

- Chunk `.tscn` files are lightweight loader scenes.
- Geometry lives in `mesh_data/Chunk_X_Z.json`.
- `scripts/chunk_mesh_loader.gd` reads chunk JSON on a thread, batches meshes by material and creates metadata markers.
- `scripts/world_streamer.gd` loads/unloads nearby chunks from `world_manifest.json`.
- `scripts/navigation_controller.gd` reads local `navigation_index.json` and `navigation_graph.json`; it must not call external routing/search APIs.
- `scripts/fps_player.gd` provides FPS movement, noclip, control pause/resume and auto-move hooks.
- `scenes/master.tscn` creates sky, sun, cloud sprites, world floor, player, streamer and navigation controller.
- `metadata.json` currently records scale/chunk/coordinate-system data; geo bounds are still placeholders from `save_all()`.

## CLI / Output Contracts

- `--bbox` is required unless tests instantiate internals directly.
- `--file` uses local OSM JSON and bypasses Overpass.
- `--save-json-file` saves downloaded OSM JSON for reuse.
- `--output-dir` has alias `--path`.
- `--chunk-size` is in arnis block units; Godot size is `chunk-size * godot-scale`.
- `--fetch-tile-degrees` default is `0.04`; README examples may deliberately use larger values for Shanghai.
- `--land-cover` defaults to true because `clap` uses a bool default, but there is currently no `--no-land-cover` flag.
- `project.godot` targets Godot 4.6 and should not contain `"4.3"`.

## Code Style

- Language: Rust 2021.
- Keep generated Godot text resources ASCII unless there is a clear product reason otherwise. Generated runtime scripts may contain Chinese UI text where already present.
- Prefer small, explicit functions over broad rewrites.
- Keep behavior changes covered by focused tests in the same module when practical.
- Use `rg` for repository search.
- Use `apply_patch` for manual edits.
- Do not run broad formatting just to touch unrelated files; preserve the existing diff scope.
- Generated Godot scripts should remain Godot 4.6 compatible.
- Avoid adding dependencies for tasks that can be done with the existing Rust/Godot standard library surface.

## Safety & Conventions

- Do not modify `refs/arnis/`.
  - Why: it is the upstream/reference implementation used for comparison.
  - Do instead: read it, copy ideas into `src/`, and cite the source path in notes.
  - Verify: `git diff --name-only` must not include `refs/arnis/`.

- Do not write build output into the repo unless explicitly requested.
  - Why: `target/` and generated Godot projects are large and noisy.
  - Do instead: use `--target-dir E:\tmp\osm-godot-target` and output projects under `E:\tmp\...`.
  - Verify: `git status --short` should show only intentional source/doc changes.

- Do not use destructive cleanup commands without explicit confirmation.
  - Why: generated projects and cached OSM files may be useful for comparison.
  - Do instead: create a new versioned output directory such as `E:\tmp\osm-godot-xian-yanta-style-v5-10x`.
  - Verify: no `Remove-Item -Recurse -Force` unless the user approved the exact path.

- Do not skip tests for scene-generation changes.
  - Why: most bugs surface as Godot text/resource contract mismatches.
  - Do instead: add focused tests under `src/scene_writer/*` or `src/element_processing/*` first, then run targeted and full tests.
  - Verify: `cargo test --target-dir E:\tmp\osm-godot-target` passes, or explicitly report why it was not run.

- Do not assume Godot is on PATH.
  - Why: this machine may not expose `godot`/`godot4` to shell sessions.
  - Do instead: run `where.exe godot; where.exe godot4` before claiming PATH-based headless verification, or use the known absolute Godot path if present.
  - Verify: if Godot is unavailable, state that only Rust/text-level verification was performed.

- Do not make navigation dependent on network services.
  - Why: v3 contract is fully local routing/search.
  - Do instead: use `navigation_index.json`, `navigation_graph.json` and local A* logic.
  - Verify: `navigation_controller_uses_local_graph_and_has_no_network_api` remains passing and generated script contains no `HTTPRequest`, `HTTPClient`, `WebSocketPeer`, `http://` or `https://`.

## Security

- Never commit secrets, API tokens, `.env`, private certificates, APNs credentials, or local proxy credentials.
- Live OSM/Overpass requests may need `HTTP_PROXY` / `HTTPS_PROXY` set to `http://127.0.0.1:7897`.
- Large bbox requests can hit service limits. Prefer cached `--file` inputs for iteration, and use `--tiled-fetch` with `--tile-cache-dir` for city-scale jobs.
- Generated `navigation_index.json` can contain real OSM names/addresses from public data; do not mix it with private user data.

## Testing Strategy

Use TDD for behavior changes. Write a failing test first, confirm the failure, then implement.

- Full test: `cargo test --target-dir E:\tmp\osm-godot-target`
- Scene writer tests: `cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture`
- Project writer tests: `cargo test --target-dir E:\tmp\osm-godot-target scene_writer::project_writer::tests -- --nocapture`
- Material writer tests: `cargo test --target-dir E:\tmp\osm-godot-target scene_writer::tres_writer::tests -- --nocapture`
- Navigation tests: `cargo test --target-dir E:\tmp\osm-godot-target navigation -- --nocapture`
- Building inspection tests: `cargo test --target-dir E:\tmp\osm-godot-target building_inspection`
- Whitespace check: `git diff --check`

Generated project smoke checks are usually text-level unless Godot is available:

```powershell
rg -n "Sprite3D|cloud_billboard|noclip_toggle|Player|floor_snap_length|world_streamer|navigation_controller|BuildingInspectPanel|diffuse_mode = 3" E:\tmp\<project>\scenes\master.tscn E:\tmp\<project>\scripts E:\tmp\<project>\project.godot E:\tmp\<project>\materials
```

Godot E2E scripts:

- Player/metadata: `tools\godot_player_e2e.gd`
- Streaming: `tools\godot_streaming_e2e.gd`
- Streaming performance: `tools\godot_streaming_perf_probe.gd`
- Navigation: `tools\godot_navigation_e2e.gd`
- Auto-run: `tools\godot_navigation_autorun_e2e.gd`
- Building inspection: `tools\godot_building_inspection_e2e.gd`

Known absolute console path used in prior verification:

```powershell
& 'E:\Godot_v4.6-stable_win64.exe\Godot_v4.6-stable_win64_console.exe' --headless --path E:\tmp\<project> --import --quit
```

## Godot Output Expectations

- `project.godot` targets Godot 4.6 Forward Plus and run scene is `res://scenes/master.tscn`.
- `master.tscn` contains `WorldEnvironment`, `DirectionalLight3D`, visible sun disk, cloud billboards, `WorldFloor`, `Player`, `WorldStreamer`, and `NavigationController`.
- FPS controls include WASD, arrow keys, jump, sprint, mouse capture toggle, descend, noclip, and fallbacks for direct key polling.
- Navigation controls include `N` panel toggle, green route ribbon, destination circle, local A* graph loading, `G` auto-run and arrival cleanup.
- Building inspection includes `F`, `BuildingInspectPanel`, view-cone scoring and 5 second auto-hide.
- Materials should avoid whitebox defaults and use toon-ish settings where supported by Godot 4.6.
- Terrain/road/building mesh data should exist in `mesh_data/` and be loaded through `chunk_mesh_loader.gd`.
- Metadata markers should preserve raw `osm_metadata` and sanitized direct meta keys for colon-containing OSM tags.

## Documentation Policy

- Update `README.md` when CLI flags, examples, generated output, controls, or user-facing capabilities change.
- Update `AGENTS.md` when commands, output structure, Godot version, workflow assumptions, safety rules, or verification strategy change.
- Keep `README.md` human-oriented; put agent-specific operational details here.
- If `CLAUDE.md` and `AGENTS.md` disagree, update both or explicitly state which file is stale in the final response.
- Keep `docs/prd/` and `docs/plan/` aligned when working inside a versioned milestone chain.

## Completion Notes

When a user-facing task is complete for this repo, send a short APNs notification:

```powershell
apn-pushtool send --title "osm-godot" --body "<10字以内梗概>完成"
```

Do not print or store APNs secrets.

## Scope & Precedence

- This root `AGENTS.md` applies to the whole repository.
- A more specific `AGENTS.md` in a subdirectory overrides this file for that subtree.
- `AGENTS.override.md` in the same directory takes priority over `AGENTS.md`.
- The user's explicit chat instructions override repository notes.
- Global `~/.codex/AGENTS.md` can add personal defaults, but should not contradict project-specific rules.
