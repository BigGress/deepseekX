#!/bin/bash
# ============================================
# Computer Use 工具 - 窗口管理模块
# ============================================

ACTION="${1:-list}"

case "$ACTION" in
    list)
        osascript -e "
        tell application \"System Events\"
            set output to {}
            repeat with p in (every process whose visible is true)
                try
                    tell p
                        repeat with w in every window
                            set end of output to {name:name of w, app:name, pos:position of w, size:size of w}
                        end repeat
                    end tell
                end try
            end repeat
            return output
        end tell" 2>&1
        ;;
    focus)
        osascript -e "tell application \"${2}\" to activate" 2>&1
        ;;
    geo)
        APP="${2:-}"
        osascript -e "
        tell application \"System Events\"
            tell process \"$APP\"
                set w to window 1
                return {name:name of w, pos:position of w, size:size of w}
            end tell
        end tell" 2>&1
        ;;
    setpos)
        osascript -e "
        tell application \"System Events\"
            tell process \"${2}\"
                set position of window 1 to {${3}, ${4}}
            end tell
        end tell" 2>&1
        ;;
    setsize)
        osascript -e "
        tell application \"System Events\"
            tell process \"${2}\"
                set size of window 1 to {${3}, ${4}}
            end tell
        end tell" 2>&1
        ;;
    *)
        echo "用法: $0 <list|focus|geo|setpos|setsize> [app] [args...]"
        exit 1
        ;;
esac

echo ""
