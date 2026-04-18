# IRC 连接生命周期

## 概述

一个 IRC 客户端通过 TCP 连接到网关后，经历以下阶段：

1. **连接建立**：客户端发起 TCP 连接，网关接受连接并创建 IRC 编解码器。
2. **注册协商**：客户端发送 CAP LS / CAP REQ / CAP END 和 NICK / USER 命令。
   - 必须同时收到 NICK 和 USER 才算注册完成。
   - CAP 协商期间不会触发注册，即使已经收到 NICK 和 USER。
   - 注册完成后，网关发送欢迎消息序列（001-005、MOTD）。
3. **正常通信**：客户端可以 JOIN 频道（对应 QQ 群）、PRIVMSG 发消息、执行各种 IRC 查询命令。
4. **断开连接**：客户端发送 QUIT 或 TCP 连接断开。

## 状态模型

```
[新连接] → Pending { got_nick: false, got_user: false }
                │
    收到 NICK ──┤── 收到 USER
                │
    Pending { got_nick: true, got_user: true }
                │
        (cap_negotiating == false)
                │
          Registered
                │
         正常 IRC 会话
```

## 消息流向

```
IRC 客户端 ──TCP──→ [IRC 编解码器] ──Message──→ [命令分发器]
                                                    │
                                        ┌───────────┤
                                        ↓           ↓
                                  [纯协议构建]  [OneBot API 调用]
                                        │           │
                                        └─────┬─────┘
                                              ↓
                                    [发送 IRC 回复消息]

OneBot 事件 ──broadcast──→ [渲染为 IRC 消息] ──→ [发送到 IRC 客户端]
```
