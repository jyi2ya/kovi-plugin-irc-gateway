========
如何使用
========

1) 运行 `cargo run --example irc-gateway`，程序会询问 onebot 服务的相关信息。**目前只支持正向 websocket 协议[1]！**
2) irc 服务器默认会监听 0.0.0.0:8621 端口。可以在 `data/kovi-plugin-irc-gateway/config.toml` 里面修改
3) 把 irc 客户端的 **NICKNAME** 改成 `rivus` （其它名字可能工作也可能不工作，不知道）
4) 开始聊天

[1]: https://www.napcat.wiki/onebot/network#_2-2-napcatqq-%E4%BD%9C%E4%B8%BA-websocket-%E6%9C%8D%E5%8A%A1%E5%99%A8%E6%8E%A5%E6%94%B6%E4%BA%8B%E4%BB%B6%E5%92%8C%E8%AF%B7%E6%B1%82

========
配置文件
========

配置文件为 `data/kovi-plugin-irc-gateway/config.toml`，配置文件示例：

    bind_addr = "0.0.0.0:8621"

（就一行）

====
谢谢
====

* ThriceCola/Kovi: https://github.com/ThriceCola/Kovi
* aatxe/irc: https://github.com/aatxe/irc
