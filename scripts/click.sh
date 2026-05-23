#!/bin/bash
# ============================================
# Computer Use 工具 - 鼠标点击模块
# 依赖: macOS osascript + System Events (需辅助功能权限)
# ============================================

ACTION="${1:-click}"
X="${2:-0}"
Y="${3:-0}"

case "$ACTION" in
    click)
        osascript -e "tell application \"System Events\" to click at {$X, $Y}" 2>&1
        ;;
    doubleclick)
        osascript -e "tell application \"System Events\" to double click at {$X, $Y}" 2>&1
        ;;
    rightclick)
        osascript -e "tell application \"System Events\" to click at {$X, $Y} with right button" 2>&1
        ;;
    move)
        osascript -e "tell application \"System Events\" to mouse move at {$X, $Y}" 2>&1
        ;;
    *)
        echo "用法: $0 <click|doubleclick|rightclick|move> <x> <y>"
        exit 1
        ;;
esac

if [ $? -eq 0 ]; then
    echo "OK: $ACTION at ($X, $Y)"
else
    echo "ERROR: 操作失败。请检查「系统设置 > 隐私与安全性 > 辅助功能」是否已授权。"
    exit 1
fi
