#!/bin/bash
# =============================================================================
# Computer Use - 主协调器
# 使用截图 + 视觉分析 + 键鼠模拟来操控桌面应用
#
# 用法:
#   ./scripts/computer_use.sh <任务描述>
#
# 示例:
#   ./scripts/computer_use.sh "点击新建对话按钮"
#   ./scripts/computer_use.sh "在输入框输入Hello World并发送"
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ---------- 参数 ----------
TASK="$*"
if [ -z "$TASK" ]; then
    echo "用法: $0 <任务描述>"
    echo '示例: ./scripts/computer_use.sh "点击侧边栏的新建对话按钮"'
    exit 1
fi

# ---------- 步骤1: 截图 ----------
SCREENSHOT="/tmp/deepseekx_screenshot.png"
echo ">>> [1/4] 截图..."
bash "$SCRIPT_DIR/screenshot.sh" "$SCREENSHOT"
if [ $? -ne 0 ]; then
    echo "截图失败，终止。"
    exit 1
fi

# ---------- 步骤2: 获取窗口信息 ----------
echo ">>> [2/4] 获取窗口信息..."
WINDOWS=$(bash "$SCRIPT_DIR/window.sh" list 2>/dev/null)
if [ $? -ne 0 ]; then
    echo "警告: 无法获取窗口信息。"
fi

# ---------- 步骤3: 视觉分析 ----------
echo ">>> [3/4] 视觉分析（需要 DeepSeek CLI）..."

# 构建分析提示
ANALYSIS_PROMPT="你是一个桌面自动化助手。分析这张截图，完成以下任务:

任务: $TASK

当前可见窗口:
$WINDOWS

请返回 JSON 格式的操作序列:
{
  \"steps\": [
    {\"action\": \"click\", \"x\": 数字, \"y\": 数字, \"description\": \"说明\"},
    {\"action\": \"type\", \"text\": \"要输入的文字\", \"description\": \"说明\"},
    {\"action\": \"key\", \"code\": 36, \"description\": \"按回车\"}
  ]
}

注意:
- 坐标(x,y)是相对于屏幕左上角的像素坐标
- 如果任务涉及按钮，给出按钮中心点的坐标
- 如果看不到目标元素，返回空 steps 数组
- 只返回 JSON，不要其他内容"

# 调用 DeepSeek 分析截图 (多模态)
if command -v deepseek &>/dev/null; then
    RESPONSE=$(deepseek -p "$ANALYSIS_PROMPT" --image "$SCREENSHOT" 2>/dev/null)
else
    echo "ERROR: 未找到 deepseek CLI"
    exit 1
fi

echo "分析结果: $RESPONSE"

# ---------- 步骤4: 执行操作 ----------
echo ">>> [4/4] 执行操作..."

# 解析 JSON 并执行（使用 python 辅助）
echo "$RESPONSE" | python3 -c "
import sys, json

# 从输出中提取 JSON 块
text = sys.stdin.read()

# 尝试在文本中找 JSON
start = text.find('{')
end = text.rfind('}') + 1
if start == -1:
    print('未找到 JSON 响应')
    sys.exit(1)

try:
    data = json.loads(text[start:end])
except json.JSONDecodeError as e:
    print(f'JSON 解析失败: {e}')
    print(f'原始文本: {text}')
    sys.exit(1)

steps = data.get('steps', [])
if not steps:
    print('无操作步骤')
    sys.exit(0)

import subprocess, os
script_dir = '$SCRIPT_DIR'

for i, step in enumerate(steps):
    action = step.get('action', '')
    desc = step.get('description', '')
    print(f'[{i+1}/{len(steps)}] {desc}')

    if action == 'click':
        x, y = step.get('x', 0), step.get('y', 0)
        subprocess.run(['bash', f'{script_dir}/click.sh', 'click', str(x), str(y)])
    elif action == 'doubleclick':
        x, y = step.get('x', 0), step.get('y', 0)
        subprocess.run(['bash', f'{script_dir}/click.sh', 'doubleclick', str(x), str(y)])
    elif action == 'type':
        text = step.get('text', '')
        subprocess.run(['bash', f'{script_dir}/type.sh', 'text', text])
    elif action == 'key':
        code = step.get('code', 36)
        subprocess.run(['bash', f'{script_dir}/type.sh', 'key', str(code)])
    elif action == 'shortcut':
        mod = step.get('modifier', 'command down')
        key = step.get('key', '')
        subprocess.run(['bash', f'{script_dir}/type.sh', 'shortcut', mod, key])
    elif action == 'sleep':
        import time
        time.sleep(step.get('seconds', 0.5))
    elif action == 'focus':
        app = step.get('app', '')
        subprocess.run(['bash', f'{script_dir}/window.sh', 'focus', app])
    else:
        print(f'  未知操作: {action}')

print('操作完成')
"
echo ""
echo "=== Computer Use 任务完成 ==="
