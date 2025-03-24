# LokiPool 测试工具

本目录包含用于测试和诊断 LokiPool 的实用工具。

## test_proxy.sh

测试代理连接和SOCKS服务器功能的脚本。

### 使用方法

```bash
# 使工具可执行
chmod +x tools/test_proxy.sh

# 使用默认设置运行
./tools/test_proxy.sh

# 指定代理地址和端口
./tools/test_proxy.sh -p 127.0.0.1:12333

# 指定SOCKS服务器端口和测试URL
./tools/test_proxy.sh -s 1080 -u http://www.baidu.com

# 指定超时时间
./tools/test_proxy.sh -t 30

# 显示帮助信息
./tools/test_proxy.sh -h
```

### 参数说明

- `-p HOST:PORT`: 指定上游代理地址和端口
- `-s PORT`: 指定LokiPool SOCKS服务器端口
- `-u URL`: 指定测试目标URL
- `-t SECONDS`: 指定超时时间（秒）
- `-h`: 显示帮助信息

## 调试

在运行 LokiPool 时，可以通过设置环境变量 `RUST_LOG=lokipool=debug` 来启用 `debug` 日志级别，以获取更详细的日志输出。例如：

```bash
RUST_LOG=lokipool=debug ./target/release/lokipool
```

这将输出所有 `debug`、`info`、`warn` 和 `error` 级别的日志信息，有助于诊断问题。
