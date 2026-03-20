# gRPC 协议配置

gRPC 是一个高性能、开源的远程过程调用 (RPC) 框架，基于 HTTP/2 协议传输。

## 特性

- 基于 HTTP/2 的多路复用
- 高效的二进制序列化 (Protocol Buffers)
- 支持双向流
- 跨语言支持

## 配置示例

### 服务端配置

```json
{
  "listen": {
    "addr": "0.0.0.0:50051",
    "net": "tcp",
    "trans": {
      "proto": "grpc",
      "path": "/roma.Relay/Connect"
    }
  },
  "remote": "127.0.0.0:8322"
}
```

### 客户端配置

```json
{
  "listen": "0.0.0.0:8320",
  "remote": {
    "addr": "server.example.com:50051",
    "net": "tcp",
    "trans": {
      "proto": "grpc",
      "path": "/roma.Relay/Connect"
    }
  }
}
```

## 高级配置

```json
{
  "trans": {
    "proto": "grpc",
    "path": "/roma.Relay/Connect",
    "mux": 100
  }
}
```

### 参数说明

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| path | string | - | gRPC 服务路径 |
| mux | int | 1000 | 最大并发连接数 |

## 使用场景

gRPC 协议特别适合以下场景：

1. **微服务架构**：服务间通信
2. **跨语言通信**：多语言服务互通
3. **高性能需求**：相比 REST 更高效
4. **双向流**：实时数据传输

## 注意事项

1. gRPC 基于 HTTP/2，需要 TCP 连接
2. 服务路径必须匹配服务端定义
3. 建议配合 TLS 使用以保证安全性

## 编译支持

使用 `--features grpc` 启用 gRPC 支持：

```bash
cargo build --features grpc
```

或使用完整特性：

```bash
cargo build --features full
```
