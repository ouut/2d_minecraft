# 2.5D 分层体素引擎 (Dwarf Fortress × Minecraft)

这是一个基于 **Rust + Macroquad + hecs + steel-core (Scheme)** 开发的高性能 2.5D 等距视角分层体素游戏引擎。它融合了《矮人堡垒》(Dwarf Fortress) 的分层切片浏览、殖民地 AI 调度与《Minecraft》的 3D 方块世界、程序化地形、流体、光照与物理模拟，并内置了基于 **Steel Scheme 的 Mod 脚本语言与交互式 REPL 控制台**。

---

## 🌟 核心特性

1. **三维数据与 2.5D 等距切片渲染**
   - 真正的 3D 体素存储：每个体素 4 字节紧凑编码，支持 64 层垂直深度与无限水平分块动态加载。
   - 2:1 经典等轴测 (Isometric) 投影，**支持 4 方向 90° 顺时针自由旋转视角**（`R` 键）。
   - **矮人堡垒风格 Z 轴切片浏览**：通过 `Q/E` 或鼠标滚轮平滑升降当前编辑切片层 `slice_z`，自动剖切隐藏头顶遮挡物，深层具备层次分明的蓝紫色深度雾化。
   - 高效遮挡剔除 (Occlusion Culling) 与 Back-to-Front 画家算法批次渲染。

2. **三大玩法模式无缝切换**
   - **`F1` 主角生存探索模式 (Player Mode)**：相机平滑跟随主角，`WASD` 沿视角方向移动（遇斜坡自动上下台阶），鼠标左键挖掘方块、右键放置方块。
   - **`F2` 殖民地 AI 调度模式 (Colony Mode)**：自由平移相机与切片，鼠标框选作业区下达【挖掘】、【挖渠】、【建造墙体/斜坡/楼梯】指令，矮人殖民者通过 **3D A\* 跨层寻路** 自动认领最近任务并施工。
   - **`F3` 上帝世界编辑器 (God Mode)**：自由调色板、倾倒水流与岩浆、一键调用由 Scheme Mod 脚本编写的自定义结构生成器与魔法笔刷。

3. **核心物理与环境模拟**
   - **双通道 BFS 光照传播**：天空光 `SkyLight (0-15)`（垂直投射向下漫射入洞穴，树叶与水衰减） + 方块光 `BlockLight (0-15)`（火把、萤石、岩浆与自定义发光矿石向 6 邻域漫射）。
   - **8 级元胞自动机流体**：水与岩浆优先垂直下坠，受阻向四周平铺；流动水接触岩浆源生成黑曜石 (`Obsidian`)，接触流动岩浆生成圆石 (`Cobblestone`)。
   - **重力与悬空坍塌**：沙子、沙砾下方无支撑时自然下坠，遇坡道沿休止角滑落；挖掘引发的局部悬空结构会在超出支撑极限 (`MAX_CANTILEVER = 5`) 时断裂坍塌。

4. **Steel (Scheme) Mod 脚本引擎与开发工作流**
   - 采用高效嵌入式 Lisp 引擎 `steel-core`。
   - **动态方块注册**：无需重新编译 Rust，在 `.scm` 脚本中调用 `(register-block! ...)` 即可注册新方块并自动生成 2.5D 像素瓦片加入调色板。
   - **一键热重载 (Hot Reload)**：游戏中按 `F5` 即可实时重载 `mods/*.scm`，无需重启游戏。
   - **游戏内交互式 REPL 控制台**：按 `~`（反引号）或 `F12` 唤出半透明终端，在游戏内直接运行 Scheme 表达式实时操纵世界与实体。
   - **自带官方示例 Mod 包 (`mods/`)**：
     - `00_stdlib.scm`：几何建造宏与开掘辅助库。
     - `01_magic_blocks.scm`：注册秘银矿 (`mythril_ore`)、以太水晶 (`aether_crystal`)、流沙 (`quicksand`) 等奇幻方块。
     - `02_structures_and_tools.scm`：注册【建造瞭望塔】、【陨石天降】、【召唤殖民队】等自定义上帝笔刷。

---

## 🎮 操作快捷键表

| 按键 / 操作 | 功能描述 |
| :--- | :--- |
| **`F1`** | 切换至 **主角生存探索模式** (Player Mode) |
| **`F2`** | 切换至 **殖民地 AI 调度模式** (Colony Mode) |
| **`F3`** | 切换至 **上帝世界编辑模式** (God Editor Mode) |
| **`WASD` / 方向键** | 移动主角（遇自然坡道自动攀爬）或平移自由摄像机 |
| **`R`** | 2.5D 等距视角 **90° 顺时针旋转** (0° N $\rightarrow$ 90° E $\rightarrow$ 180° S $\rightarrow$ 270° W) |
| **`Q` / `E` 或 鼠标滚轮** | 调节当前 **Z 轴剖切高度 (`slice_z`)**，剖切上层方块 |
| **鼠标左键** | 挖掘方块 / 框选殖民地任务 / 使用上帝工具笔刷 |
| **鼠标右键** | 放置当前选中的方块 / 取消框选 |
| **`F5`** | **热重载 Steel Scheme Mod 脚本** (`mods/*.scm`) |
| **`~` (反引号) / `F12`** | 打开 / 关闭游戏内 **交互式 Scheme REPL 控制台** |
| **`ESC`** | 关闭 REPL 控制台 |

---

## 🏗️ 架构设计 (Strict 3-Tier Star-Tree DAG)

本项目为了支持多 AI Agent 并发协同开发且互不打架，将 Cargo Workspace 严格设计为单向无环星型树状依赖架构：

```
/workspace/2d_minecraft/
├── Cargo.toml                       # [Phase 0 冻结] Workspace 根配置与统一依赖版本
├── docs/                            # 详细设计与并发手册
│   ├── implementation_plan.md       # 总体架构设计蓝图
│   └── agent_concurrency_workflow.md# 多 Agent 协同并发工作手册
├── mods/                            # 官方示例 Scheme Mod 脚本
│   ├── 00_stdlib.scm
│   ├── 01_magic_blocks.scm
│   └── 02_structures_and_tools.scm
└── crates/
    ├── voxel_core/                  # [Layer 0 底座契约] 纯数据结构、Trait与消息总线 (零重依赖)
    ├── voxel_gen/                   # [Layer 1 正交叶节点] MC 3D 密度噪声地形、生物群系与矿脉
    ├── voxel_sim/                   # [Layer 1 正交叶节点] 双通道 BFS 光照、水/岩浆流体与坍塌
    ├── voxel_ecs/                   # [Layer 1 正交叶节点] hecs 实体、3D 跨层 A* 寻路与殖民地作业
    ├── voxel_script/                # [Layer 1 正交叶节点] Steel Scheme VM、热重载与 REPL 终端
    ├── voxel_render/                # [Layer 1 正交叶节点] 程序化 2.5D 像素图集与画家算法渲染器
    └── voxel_app/                   # [Layer 2 聚合宿主] 主循环、分块容器、输入分发与窗口入口
```

---

## 🚀 本地运行与测试

### 1. 一键本地构建脚本 (`build.sh`)
项目根目录提供了自动化编译与打包脚本，一次性构建所有桌面端原生程序与 WebAssembly 客户端：
```bash
./build.sh
```
该脚本会自动：
- 编译 3 款桌面原生程序（`voxel_app`、`voxel_web`、`voxel_server`）至 `target/release/`
- 编译 WebAssembly 3D 客户端 `voxel_web.wasm` 并同步更新至 `assets/web/`
- 打包运行所需静态资源与 Mod 至 `dist/`

---

### 2. Cargo 启动命令大全

| 目标程序 / 服务 | 启动命令 | 默认端口 / 地址 | 说明 |
| :--- | :--- | :--- | :--- |
| **`voxel_server` (网关服务端)** | `cargo run --release --bin voxel_server` | **`0.0.0.0:8083`** | 启动单端口 WebAssembly 静态资源托管与 WebSocket 网关 |
| **`voxel_server` (自定义端口)** | `PORT=9000 cargo run --release --bin voxel_server` | `0.0.0.0:9000` | 通过 `PORT` 环境变量动态更改服务端监听端口 |
| **`voxel_app` (2.5D 切片引擎)** | `cargo run --release --bin voxel_app` | 桌面窗口 (1280x720) | 矮人堡垒风格 Z 轴切片 + Scheme Mod VM + 殖民地 AI |
| **`voxel_web` (3D 客户端原生版)** | `cargo run --release --bin voxel_web` | 桌面窗口 (1280x720) | 原汁原味 Minecraft 3D 自由视角 + `V` 键双视角切换 + AO 光影 |
| **`voxel_web` (WebAssembly 构建)** | `cargo build --release --target wasm32-unknown-unknown -p voxel_web` | `assets/web/` | 编译轻量 WebAssembly 产物（配合服务端浏览器秒开） |

#### 快速畅玩指南：
1. **启动网关服务端**：
   ```bash
   cargo run --release --bin voxel_server
   ```
2. **在浏览器中打开 3D 体素世界**：
   访问 **`http://localhost:8083/`** 即可在浏览器全屏畅玩！
3. **或直接启动桌面客户端**：
   ```bash
   cargo run --release --bin voxel_app   # 体验 2.5D 切片与 Scheme Mod
   cargo run --release --bin voxel_web   # 体验第一人称真 3D
   ```

---

### 3. 运行全量单元与集成测试
```bash
cargo test --workspace
```
> 全面验证包含 11 个 Crate 的全部 70+ 项单元测试（坐标投影变换、3D 跨层寻路、流体元胞扩散、双通道光照、Scheme 虚拟机看门狗与自适应双模网络协议）。

---

## 📦 多平台自动编译与 Release 发布 (GitHub Actions)

本项目配置了完善的 GitHub Actions 持续集成与发布工作流 (`.github/workflows/release.yml`)，可在 GitHub 仓库中全自动编译并在 Release 页面发布三大平台的可执行包与 WebAssembly 离线包：

- **Windows**: `x86_64-pc-windows-msvc` (`voxel_suite-windows-x86_64.zip`)
- **macOS (Apple Silicon)**: `aarch64-apple-darwin` (`voxel_suite-macos-arm64.tar.gz`)
- **Linux**: `x86_64-unknown-linux-gnu` (`voxel_suite-linux-x86_64.tar.gz`)
- **WebAssembly**: 浏览器独立完整运行包 (`voxel_web-wasm.zip`)

### 触发自动发布：
只需在本地打上版本 tag 并推送到 GitHub：
```bash
git tag v0.2.0
git push origin v0.2.0
```
或者在 GitHub 仓库的 **Actions** 标签页中手动点击 **Run workflow**。GitHub Actions 将自动打包三大二进制文件、WASM 产物与 `mods/` 示例目录并创建 Release。
