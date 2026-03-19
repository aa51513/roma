# KCP 协议配置

KCP 是一个快速可靠的 ARQ（自动重传请求）协议，基于 UDP 实现，提供类似 TCP 的可靠传输，但具有更低的延迟。

## 特性

- 基于 UDP 的可靠传输
- 低延迟，适合游戏、实时通信等场景
- 可配置的重传策略
- 支持拥塞控制

## 配置示例

### 服务端配置

```json
{
  "listen": {
    "addr": "0.0.0.0:8321",
    "net": "udp",
    "trans": {
      "proto": "kcp"
    }
  },
  "remote": "127.0.0.1:8322"
}
```

### 客户端配置

```json
{
  "listen": "0.0.0.0:8320",
  "remote": {
    "addr": "server.example.com:8321",
    "net": "udp",
    "trans": {
      "proto": "kcp"
    }
  }
}
```

## 高级配置

KCP 支持以下参数调整：

```json
{
  "trans": {
    "proto": "kcp",
    "nodelay": 1,
    "interval": 20,
    "resend": 2,
    "nc": false
  }
}
```

### 参数说明

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| nodelay | int | 1 | 是否启用 nodelay 模式 |
| interval | int | 20 | 内部更新间隔（毫秒） |
| resend | int | 2 | 快速重传阈值 |
| nc | bool | false | 是否关闭拥塞控制 |

## 使用场景

KCP 协议特别适合以下场景：

1. **游戏服务器**：低延迟要求
2. **实时通信**：音视频传输
3. **弱网环境**：移动网络、跨运营商
4. **长距离传输**：跨国网络连接

## 注意事项

1. KCP 基于 UDP，需要确保防火墙允许 UDP 流量
2. 在高丢包网络环境下，KCP 的带宽消耗会增加
3. 建议根据实际网络环境调整参数

## 编译支持

使用 `--features kcp` 启用 KCP 支持：

```bash
cargo build --features kcp
```

或使用完整特性：

```bash
cargo build --features full
```
