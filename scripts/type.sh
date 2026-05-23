#!/bin/bash
# ============================================
# Computer Use 工具 - 键盘输入模块
# 依赖: macOS osascript (需辅助功能权限)
# ============================================

MODE="${1:-text}"
INPUT="$2"

case "$MODE" in
    text)
        osascript -e "tell application \"System Events\" to keystroke \"$INPUT\"" 2>&1
        ;;
    key)
        # 特殊按键码: 36=return 48=tab 49=space 51=delete 53=escape
        # 123=left 124=right 125=down 126=up
        osascript -e "tell application \"System Events\" to key code $INPUT" 2>&1
        ;;
    shortcut)
        MODIFIER="${2:-command down}"
        KEY="$3"
        osascript -e "tell application \"System Events\" to keystroke \"$KEY\" using $MODIFIER" 2>&1
        ;;
    *)
        echo "用法: $0 <text|key|shortcut> <input> [modifier] [key]"
        echo "  text 'hello'              - 输入文本"
        echo "  key 36                    - 特殊按键(key code)"
        echo "  shortcut 'command down' w - 组合键(关闭窗口)"
        exit 1
        ;;
esac

if [ $? -eq 0 ]; then
    echo "OK: $MODE"
else
    echo "ERROR: 输入失败。"
    exit 1
fi
