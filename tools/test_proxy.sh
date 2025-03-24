#!/bin/bash

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # 无颜色

# 默认值
PROXY_HOST="127.0.0.1"
PROXY_PORT="12333"
# TARGET_URL="http://www.baidu.com" # v6
TARGET_URL="http://www.google.com" 
SOCKS_SERVER_PORT="1080"

# 超时时间（秒）
TIMEOUT=15

# 显示帮助
show_help() {
    echo -e "${BLUE}LokiPool 代理测试工具${NC}"
    echo "用法: $0 [-p 代理主机:端口] [-s SOCKS服务器端口] [-u 目标URL] [-t 超时时间]"
    echo
    echo "选项:"
    echo "  -p    代理地址和端口 (默认: ${PROXY_HOST}:${PROXY_PORT})"
    echo "  -s    SOCKS服务器端口 (默认: ${SOCKS_SERVER_PORT})"
    echo "  -u    测试目标URL (默认: ${TARGET_URL})"
    echo "  -t    超时时间 (默认: ${TIMEOUT} 秒)"
    echo "  -h    显示此帮助信息"
    echo
}

# 解析参数
while getopts "p:s:u:ht:" opt; do
    case $opt in
        p)
            IFS=':' read -r PROXY_HOST PROXY_PORT <<< "$OPTARG"
            ;;
        s)
            SOCKS_SERVER_PORT="$OPTARG"
            ;;
        u)
            TARGET_URL="$OPTARG"
            ;;
        t)
            TIMEOUT="$OPTARG"
            ;;
        h)
            show_help
            exit 0
            ;;
        \?)
            echo "无效选项: -$OPTARG" >&2
            show_help
            exit 1
            ;;
    esac
done

echo -e "${BLUE}===== LokiPool 代理测试工具 =====${NC}"
echo -e "${YELLOW}测试配置:${NC}"
echo -e "  代理: ${PROXY_HOST}:${PROXY_PORT}"
echo -e "  SOCKS服务器: 127.0.0.1:${SOCKS_SERVER_PORT}"
echo -e "  目标URL: ${TARGET_URL}"
echo -e "  超时时间: ${TIMEOUT} 秒"
echo

# 测试目标可访问性
echo -e "${YELLOW}预检测: 检查目标URL是否可直接访问${NC}"
direct_result=$(timeout $TIMEOUT curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 \
    "${TARGET_URL}" 2>/dev/null)

if [ $? -eq 0 ] && [ "$direct_result" -ge 200 ] && [ "$direct_result" -lt 400 ]; then
    echo -e "  ${GREEN}✓ 目标URL可直接访问 (HTTP状态码: ${direct_result})${NC}"
else
    echo -e "  ${YELLOW}! 无法直接访问目标URL (状态码: ${direct_result})${NC}"
    echo -e "  ${YELLOW}  这可能会影响后续测试结果${NC}"
fi

# 测试1: 检查代理是否可以连接
echo -e "${YELLOW}测试1: 检查代理连接${NC}"
if timeout $TIMEOUT nc -z -w 5 "${PROXY_HOST}" "${PROXY_PORT}" 2>/dev/null; then
    echo -e "  ${GREEN}✓ 代理${PROXY_HOST}:${PROXY_PORT}可以连接${NC}"
else
    echo -e "  ${RED}✗ 无法连接到代理${PROXY_HOST}:${PROXY_PORT}${NC}"
    echo -e "  ${YELLOW}提示: 请确认代理服务器是否启动，地址和端口是否正确${NC}"
    exit 1
fi

# 测试2: 使用CURL通过SOCKS代理访问目标URL
echo -e "${YELLOW}测试2: 通过系统SOCKS代理访问目标${NC}"
echo -e "  使用代理: ${PROXY_HOST}:${PROXY_PORT}"
curl_result=$(timeout $TIMEOUT curl -s -o /dev/null -w "%{http_code}" --connect-timeout 10 \
    --socks5 "${PROXY_HOST}:${PROXY_PORT}" "${TARGET_URL}" 2>/dev/null)

if [ $? -eq 0 ] && [ "$curl_result" -ge 200 ] && [ "$curl_result" -lt 400 ]; then
    echo -e "  ${GREEN}✓ 成功通过代理访问${TARGET_URL} (HTTP状态码: ${curl_result})${NC}"
else
    echo -e "  ${RED}✗ 无法通过代理访问${TARGET_URL}${NC}"
    echo -e "  ${YELLOW}curl返回结果: ${curl_result}${NC}"
    echo -e "  ${YELLOW}提示: 检查代理是否支持SOCKS5协议，以及目标网站是否可访问${NC}"
    
    # 添加更多诊断
    echo -e "\n  ${YELLOW}进行额外诊断...${NC}"
    
    # 尝试使用HTTP而非SOCKS5
    echo -e "  尝试以HTTP代理方式连接..."
    http_result=$(timeout $TIMEOUT curl -s -o /dev/null -w "%{http_code}" --connect-timeout 10 \
        --proxy "http://${PROXY_HOST}:${PROXY_PORT}" "${TARGET_URL}" 2>/dev/null)
    
    if [ $? -eq 0 ] && [ "$http_result" -ge 200 ] && [ "$http_result" -lt 400 ]; then
        echo -e "    ${GREEN}✓ 成功通过HTTP代理访问${TARGET_URL} (HTTP状态码: ${http_result})${NC}"
    else
        echo -e "    ${RED}✗ 无法通过HTTP代理访问${TARGET_URL}${NC}"
        echo -e "    ${YELLOW}HTTP代理返回结果: ${http_result}${NC}"
    fi
fi

# 测试3: 测试LokiPool的SOCKS服务器
echo -e "${YELLOW}测试3: 测试LokiPool SOCKS5服务器${NC}"
if timeout $TIMEOUT nc -z -w 5 "127.0.0.1" "${SOCKS_SERVER_PORT}" 2>/dev/null; then
    echo -e "  ${GREEN}✓ LokiPool SOCKS服务器127.0.0.1:${SOCKS_SERVER_PORT}可以连接${NC}"
    
    # 如果SOCKS服务器可以连接，测试通过它访问目标URL
    echo -e "${YELLOW}测试4: 通过LokiPool SOCKS服务器访问目标${NC}"
    lokipool_result=$(timeout $TIMEOUT curl -s -o /dev/null -w "%{http_code}" --connect-timeout 10 \
        --socks5 "127.0.0.1:${SOCKS_SERVER_PORT}" "${TARGET_URL}" 2>/dev/null)
    
    if [ $? -eq 0 ] && [ "$lokipool_result" -ge 200 ] && [ "$lokipool_result" -lt 400 ]; then
        echo -e "  ${GREEN}✓ 成功通过LokiPool SOCKS服务器访问${TARGET_URL} (HTTP状态码: ${lokipool_result})${NC}"
    else
        echo -e "  ${RED}✗ 无法通过LokiPool SOCKS服务器访问${TARGET_URL}${NC}"
        echo -e "  ${YELLOW}可能的原因:${NC}"
        echo -e "    1. LokiPool无法正确连接到上游代理"
        echo -e "    2. 代理链中的某个环节出现问题"
        echo -e "    3. 目标URL可能被屏蔽或不可访问"
    fi
else
    echo -e "  ${RED}✗ 无法连接到LokiPool SOCKS服务器127.0.0.1:${SOCKS_SERVER_PORT}${NC}"
    echo -e "  ${YELLOW}提示: 请确认LokiPool正在运行，并监听在正确的端口${NC}"
fi

echo
echo -e "${BLUE}测试完成${NC}"
