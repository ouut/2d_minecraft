# NoCraft 网络与网关模块平移指南 (Porting Guide to NoCraft)

> **目标读者**：负责 `/workspace/NoCraft` 的开发与重构 Agent。  
> **核心目标**：使用官方发布的通用底层 Crate `game_net` 与 `game_server`，升级/替换 `engine-net` 与 `server-main`，实现 **Web 浏览器 WASM 第一公民客户端、单端口 HTTP+WS、全双工 WebSocket、双模自适应 Codec、Token 看门狗与 Grace Period 断线保护**。

---

## 1. 架构映射关系

`NoCraft` 的 `engine-protocol` 与通用网络引擎完美契合：

| NoCraft 概念 / 类型 | 通用底座 `game_net` / `game_server` | 运行时行为 |
| :--- | :--- | :--- |
| `C2SMessage::Input(PlayerInput)` | `TInput` (高频输入) | 进入服务端的 **Latest-Only Latch**，自动丢弃中间过时帧，确保角色移动与瞄准零排队延迟。 |
| `C2SMessage::ResizeViewport` / 业务指令 | `TCmd` (离散指令) | 进入服务端的 **FIFO Command Queue**，严格保序执行。 |
| `S2CMessage::ViewportSnapshot` / `ViewportDiff` | `TSnapshot` (状态快照) | 周期性全量/增量视口差量，定向下发给对应客户端。 |
| `S2CMessage::SystemMessage` / `ThemeDefinition` | `TEvent` (离散事件) | 主题热重载广播、战斗文字事件流下发。 |

---

## 2. 改造步骤

### 第一步：在 `crates/server-main/Cargo.toml` 中声明依赖

直接引用 crates.io 上已发布的标准版本：

```toml
[dependencies]
game_net = "0.1.0"
game_server = "0.1.0"
engine-core = { path = "../engine-core" }
engine-protocol = { path = "../engine-protocol" }
engine-logic = { path = "../engine-logic" }
tokio = { version = "1.38", features = ["full"] }
```

### 第二步：定义类型桥接

在 `crates/server-main/src/net_bridge.rs` 中：

```rust
use engine_core::PlayerInput;
use engine_protocol::{SparseCellUpdate, ThemeGlyphDef, ViewportRect};
use serde::{Deserialize, Serialize};
use game_server::GenericGameServer;

/// 离散交互命令 (窗口缩放、背包操作、技能触发)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NoCraftCommand {
    ResizeViewport { width: u16, height: u16 },
    Interact { target_x: i32, target_y: i32 },
}

/// 视口同步快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NoCraftSnapshot {
    Full {
        tick: u64,
        viewport: ViewportRect,
        cells: Vec<SparseCellUpdate>,
    },
    Diff {
        tick: u64,
        updates: Vec<SparseCellUpdate>,
    },
}

/// 离散事件广播
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NoCraftEvent {
    SystemMessage { channel: String, content: String },
    ThemeDefinition { entries: Vec<ThemeGlyphDef> },
}

/// NoCraft 专用泛型服务端定义
pub type NoCraftServer = GenericGameServer<PlayerInput, NoCraftCommand, NoCraftSnapshot, NoCraftEvent>;
```

### 第三步：在 `server-main` 中启动服务与主循环

```rust
use std::path::PathBuf;
use game_server::{ServerConfig, start_server};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 配置 HTTP 静态托管 (托管 client-wasm 产物) 与 WebSocket 路由
    let config = ServerConfig {
        bind_addr: "0.0.0.0:8080".parse()?,
        heartbeat_timeout_secs: 10,
        grace_period_secs: 60, // 掉线 60 秒内重连，保留房间/世界角色实体与视口状态
        static_dir: Some(PathBuf::from("crates/client-wasm/dist")),
    };

    let (server, _handle) = start_server::<PlayerInput, NoCraftCommand, NoCraftSnapshot, NoCraftEvent>(config).await?;
    println!("NoCraft Server 启动成功：http://0.0.0.0:8080 (WS: ws://0.0.0.0:8080/ws)");

    // 2. 游戏仿真主循环 (20 TPS)
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
    loop {
        interval.tick().await;

        // A. 获取玩家最新输入 (Latest-Only，无堆积)
        let inputs = server.drain_inputs();
        for (player_id, input) in inputs {
            // logic_engine.apply_player_input(player_id, input);
        }

        // B. 获取有序指令 (FIFO)
        let commands = server.drain_commands();
        for (player_id, cmds) in commands {
            for cmd in cmds {
                // logic_engine.apply_command(player_id, cmd);
            }
        }

        // C. 计算并下发各个玩家的视口快照/差量
        // server.send_to(player_id, ServerEnvelope::Snapshot(snapshot));
    }
}
```

---

## 3. Web 浏览器 (WASM) 客户端直连模式

1. 浏览器打开 `http://<server-ip>:8080/` 直接加载 `client-wasm` 生成的 HTML 与 WASM。
2. `client-wasm` 建立 WebSocket 连接：
   - 调试时发送 JSON 字符串，直接在浏览器 Network 标签页观察每一帧视口差量。
   - 生产环境发送 ArrayBuffer 二进制包（Postcard），享受微秒级极速反序列化。
