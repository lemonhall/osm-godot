# CLAUDE.md

本文件是 osm-godot 项目的机读协作规约。目标是让 agent 在 `E:\development\osm-godot` 里做出可运行、可验证的改动。

## 项目概览

- **osm-godot** 是一个 Rust CLI 工具，功能是将 OpenStreetMap 矢量数据 + 卫星高程数据转换为 Godot Engine 4.x 的 3D 场景文件（`.tscn` + `.tres` + `project.godot`）。
- 参考项目是 `refs/arnis/`——该项目将同样的数据转换为 Minecraft Java/Bedrock 世界。osm-godot 复用了 arnis 的输入管线（OSM 获取/解析、高程/地表分类获取），替换了输出层为 Godot 场景生成。
- 项目当前处于 **v1 初始版本**，已通过 Godot 4.6 CLI headless 验证。

## 架构

```
OSM Overpass API / 本地文件  高程数据 (AWS Terrarium / USGS 3DEP / IGN)
        │                              │
        ▼                              ▼
  retrieve_data.rs              elevation/ + elevation_data.rs
        │                              │
        ▼                              ▼
  osm_parser.rs ──→ Vec<ProcessedElement>    ground.rs (Ground { elevation + land cover })
        │                              │
        └──────────┬───────────────────┘
                   ▼
          data_processing.rs (dispatch loop)
                   │
    ┌──────────────┼──────────────┐
    ▼              ▼              ▼
 buildings.rs  highways.rs    trees.rs  ...  element_processing/
    │              │              │
    └──────────────┼──────────────┘
                   ▼
          scene_writer/mod.rs (SceneWriter)
                   │
    ┌──────────────┼──────────────┐
    ▼              ▼              ▼
chunk_grid   tscn_writer   tres_writer   project_writer
    │              │              │
    └──────────────┼──────────────┘
                   ▼
          output/{project.godot, materials/, scenes/}
```

### 模块职责

| 模块 | 来源 | 职责 |
|------|------|------|
| `coordinate_system/` | 拷贝自 arnis | geo(lat/lng) ↔ cartesian(X/Z) 坐标转换 |
| `retrieve_data.rs` | 拷贝自 arnis | Overpass API 获取 OSM JSON |
| `osm_parser.rs` | 拷贝自 arnis | OSM JSON → `ProcessedElement` 向量 |
| `elevation/` + `elevation_data.rs` | 拷贝自 arnis | 多源 DEM 瓦片获取与后处理 |
| `ground.rs` | 适配自 arnis | 高程 + ESA WorldCover 地表分类网格 |
| `land_cover.rs` | 拷贝自 arnis | ESA WorldCover 数据获取 |
| `clipping.rs` + `bresenham.rs` | 拷贝自 arnis | 多边形裁剪 + 线条栅格化 |
| `progress.rs` | 新建（stub） | CLI-only stub 替代 arnis 的 Tauri 进度模块 |
| `element_processing/` | **新建** | 将 OSM 元素转换为 3D 网格 |
| `scene_writer/` | **新建** | Godot .tscn/.tres/project.godot 生成 |
| `data_processing.rs` | **新建** | 元素处理调度循环 |
| `ground_generation.rs` | **新建** | 地形高度图网格生成 |
| `args.rs` | **新建** | Godot 专用 CLI 参数 |
| `main.rs` | **新建** | 入口 + 管道编排 |

### 输出格式

- 不使用 ArrayMesh 内联二进制数据（跨 Godot 版本脆弱），改用 Godot 内置网格体：
  - **BoxMesh**: 建筑
  - **CylinderMesh**: 树干、树冠（top_radius=0 模拟锥体）
  - **PlaneMesh**: 地形、水面、道路
- 区块粒度：`--chunk-size` Godot 单位（默认 128，即 64m×64m World 空间）
- 坐标映射：`GX = arnis_X × godot_scale`，`GZ = -arnis_Z × godot_scale`，`GY = arnis_level × godot_scale`

## 快速命令

命令默认在 bash 中执行（项目使用 bash shell）。先定义变量：

```bash
PROJECT=/e/development/osm-godot
GODOT="/e/Godot_v4.6-stable_win64.exe/Godot_v4.6-stable_win64_console.exe"
```

### 编译与测试

```bash
# 开发编译
cd $PROJECT && cargo check

# 发布编译
cargo build --release

# 运行测试（无高程下载，快速验证）
cargo run --release -- \
  --bbox="48.1360,11.5770,48.1363,11.5775" \
  --output-dir="./test_output"

# 带真实地形
cargo run --release -- \
  --bbox="37.770,-122.450,37.775,-122.445" \
  --terrain \
  --output-dir="./sf_block"

# Godot headless 验证
"$GODOT" --headless --rendering-driver dummy --path "./test_output" --quit
# 期望：exit code 0，无 ERROR
```

### Godot 编辑器打开

```bash
# 用 Godot GUI 打开生成的项目
"/e/Godot_v4.6-stable_win64.exe/Godot_v4.6-stable_win64.exe" --path "./test_output"
```

## 当前状态与限制（v1）

### 已验证
- Rust 编译通过（54 warnings，0 errors）
- OSM Overpass API 数据获取正常
- 高程数据管线完整（支持 AWS/USGS/IGN 多源）
- Godot 4.6 headless 解析通过（exit 0）

### v1 限制
- 建筑仅有简单 BoxMesh 形状（无屋顶细节）
- 不支持建筑内部结构
- 不支持 Overture Maps 补充数据
- 不支持 3D 模型导入（glTF）
- 不支持桥梁结构
- 仅 CLI，无 GUI
- 道路为平面 PlaneMesh（非弯曲带状）
- 区块间建筑/道路可能被截断（无跨区块元素处理）

### 可扩展方向
- ArrayMesh 内联数据（获得精确几何体）
- 多区块跨接元素
- 建筑屋顶细节（gabled/hipped 等）
- Overture Maps 建筑补充
- Tauri GUI
- 性能优化（大型场景网格合并）

## 安全边界

- `refs/arnis/` 是只读参考目录，不要修改
- 不要在仓库中提交真实 API 密钥或认证凭据
- 大规模 bbox 请求可能触发 Overpass API 限速
- ESA WorldCover 和 AWS Terrarium 是公共服务，注意请求频率
