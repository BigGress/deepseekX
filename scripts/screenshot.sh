#!/bin/bash
# ============================================
# Computer Use 工具 - 截图模块
# 依赖: macOS screencapture (需屏幕录制权限)
# ============================================

OUTPUT="${1:-/tmp/deepseekx_screenshot.png}"

screencapture -x -t png "$OUTPUT" 2>&1

if [ $? -eq 0 ] && [ -f "$OUTPUT" ]; then
    echo "截图已保存: $OUTPUT"
    echo "SCREENSHOT_PATH=$OUTPUT"
else
    echo "ERROR: 截图失败。请检查「系统设置 > 隐私与安全性 > 屏幕录制」是否已授权。"
    exit 1
fi
