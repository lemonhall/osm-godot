# Agent Notes (osm-godot)

本文件是 osm-godot 的 AI 协作规约。目标是让 agent 在 Windows/PowerShell 环境里做出可运行、可验证、不会误伤参考工程的改动。

## Project Overview

osm-godot 是一个 Rust CLI 工具，用 OpenStreetMap 数据生成 Godot 4.x 3D 城市场景工程。参考实现位于 `refs/arnis/`，它把 OSM 转为 Minecraft 世界；本项目复用输入/解析思路，但输出为 Godot `project.godot`、`.tscn`、`.tres`、脚本和 JSON mesh 数据。

## Quick Commands

在仓库根目录 `E:\development\osm-godot` 下运行。默认 shell 是 PowerShell。

- Check: `cargo check --target-dir E:\tmp\osm-godot-target`
- Test full: `cargo test --target-dir E:\tmp\osm-godot-target`
- Test scene writer: `cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture`
- Generate small Xi'an sample from cached OSM:
  `cargo run --target-dir E:\tmp\osm-godot-target -- --file E:\tmp\osm-godot-xian-yanta-style-osm.json --bbox "34.2160,108.9550,34.2210,108.9620" --output-dir E:\tmp\osm-godot-xian-yanta-style-vN --chunk-size 128`
- Generate live OSM data through local proxy:
  `$env:HTTP_PROXY='http://127.0.0.1:7897'; $env:HTTPS_PROXY='http://127.0.0.1:7897'; cargo run --target-dir E:\tmp\osm-godot-target -- --bbox "34.210594,108.947432,34.226406,108.969568" --output-dir E:\tmp\osm-godot-xian-yanta-style-vN --chunk-size 128`
- Diff check: `git diff --check`

Prefer `--target-dir E:\tmp\osm-godot-target` so local build artifacts do not churn inside the repo.

## Architecture Overview

```text
OSM Overpass / local JSON
        |
        v
retrieve_data.rs -> osm_parser.rs -> Vec<ProcessedElement>
                                  |
                                  v
                         data_processing.rs
                                  |
          +-----------------------+-----------------------+
          v                       v                       v
element_processing/buildings.rs  highways.rs             trees.rs
          +-----------------------+-----------------------+
                                  v
                           scene_writer/mod.rs
          +-----------------------+-----------------------+
          v                       v                       v
   chunk_grid.rs            tscn_writer.rs          tres_writer.rs
          |                       |                       |
          +-----------------------+-----------------------+
                                  v
 output project: project.godot, scenes/, materials/, scripts/, mesh_data/, assets/
```

Important modules:

- `src/main.rs`: CLI entry and pipeline orchestration.
- `src/args.rs`: Godot-specific CLI arguments.
- `src/data_processing.rs`: dispatch loop from parsed OSM elements into element processors.
- `src/element_processing/`: OSM element to scene mesh conversion.
- `src/ground_generation.rs`: terrain mesh generation.
- `src/scene_writer/mod.rs`: top-level Godot output orchestration, master scene, runtime scripts.
- `src/scene_writer/tscn_writer.rs`: chunk scene and `mesh_data/*.json` writing.
- `src/scene_writer/tres_writer.rs`: Godot material `.tres` writing.
- `src/scene_writer/project_writer.rs`: `project.godot` and input map writing.
- `refs/arnis/`: read-only reference project.

Current output model:

- Chunk `.tscn` files are lightweight loader scenes.
- Geometry lives in `mesh_data/Chunk_X_Z.json`.
- `scripts/chunk_mesh_loader.gd` builds `ArrayMesh` surfaces at runtime and attaches materials/collisions.
- `scenes/master.tscn` instances all non-empty chunks, creates sky/sun/clouds, and adds an FPS `CharacterBody3D` player.

## Code Style

- Language: Rust, edition from `Cargo.toml`.
- Keep generated Godot text resources ASCII unless there is a clear reason otherwise.
- Prefer small, explicit functions over broad rewrites.
- Keep behavior changes covered by tests in the same module when practical.
- Use `rg` for repository search.
- Use `apply_patch` for manual edits.
- Do not run broad formatting just to touch unrelated files; preserve the existing diff scope.
- Generated Godot scripts should remain Godot 4.6 compatible.

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
  - Do instead: add focused tests under `src/scene_writer/*` first, then run targeted and full tests.
  - Verify: `cargo test --target-dir E:\tmp\osm-godot-target` passes.

- Do not assume Godot is on PATH.
  - Why: this machine may not expose `godot`/`godot4` to shell sessions.
  - Do instead: run `where.exe godot; where.exe godot4` before claiming headless verification.
  - Verify: if not found, state that only Rust/text-level verification was performed.

## Security

- Never commit secrets, API tokens, `.env`, private certificates, or local proxy credentials.
- Live OSM/Overpass requests may need `HTTP_PROXY` / `HTTPS_PROXY` set to `http://127.0.0.1:7897`.
- Large bbox requests can hit service limits. Prefer cached `--file` inputs for iteration, and use versioned output directories for comparisons.

## Testing Strategy

Use TDD for behavior changes. Write a failing test first, confirm the failure, then implement.

- Full test: `cargo test --target-dir E:\tmp\osm-godot-target`
- Scene writer tests: `cargo test --target-dir E:\tmp\osm-godot-target scene_writer -- --nocapture`
- Project writer tests: `cargo test --target-dir E:\tmp\osm-godot-target scene_writer::project_writer::tests -- --nocapture`
- Material writer tests: `cargo test --target-dir E:\tmp\osm-godot-target scene_writer::tres_writer::tests -- --nocapture`
- Whitespace check: `git diff --check`

Generated project smoke checks are usually text-level unless Godot is available:

```powershell
rg -n "Sprite3D|cloud_billboard|noclip_toggle|Player|floor_snap_length|diffuse_mode = 3" E:\tmp\<project>\scenes\master.tscn E:\tmp\<project>\scripts E:\tmp\<project>\project.godot E:\tmp\<project>\materials
```

## Godot Output Expectations

- `project.godot` targets Godot 4.6 and should not contain `"4.3"`.
- Run scene is `res://scenes/master.tscn`.
- `master.tscn` should contain `WorldEnvironment`, `DirectionalLight3D`, visible sun, cloud billboards, `Player`, and chunk instances.
- FPS controls include WASD, arrow keys, jump, sprint, mouse capture toggle, and `noclip_toggle` for debug traversal.
- Materials should avoid whitebox defaults and use toon-ish settings where supported by Godot 4.6.
- Terrain/road mesh data should exist in `mesh_data/` and be loaded through `chunk_mesh_loader.gd`.

## Documentation Policy

- Update `AGENTS.md` when commands, output structure, Godot version, or workflow assumptions change.
- Keep `README.md` human-oriented; put agent-specific operational details here.
- If `CLAUDE.md` and `AGENTS.md` disagree, update both or state which file is stale in the final response.

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
