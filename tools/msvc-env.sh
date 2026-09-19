# 在 Git Bash 里手动搭 MSVC 命令行环境（cmd.exe / vcvarsall.bat 不可用时的替代方案）
#
# 用法：  source tools/msvc-env.sh
#
# 只影响当前 shell，不会污染系统。
# dev.sh / start-server.sh 在 cmd.exe 不可用时会自动 source 本文件。
#
# 可用环境变量覆盖自动探测结果：MSVC_BASE / MSVC_VER / SDK_ROOT / SDK_VER
# 设 MSVC_QUIET=1 可关闭加载成功的提示输出。
#
# 注：INCLUDE / LIB 必须是 Windows 风格路径（分号分隔），PATH 用 POSIX 风格（冒号分隔）。

# --- 定位 VS 2022 安装（按 BuildTools/Community/Professional/Enterprise 顺序） ---
if [ -z "${MSVC_BASE:-}" ]; then
    for b in "C:/Program Files (x86)/Microsoft Visual Studio/2022" \
             "C:/Program Files/Microsoft Visual Studio/2022"; do
        for e in BuildTools Community Professional Enterprise; do
            if [ -d "$b/$e/VC/Tools/MSVC" ]; then MSVC_BASE="$b/$e"; fi
        done
    done
fi
MSVC_BASE="${MSVC_BASE:-C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools}"
SDK_ROOT="${SDK_ROOT:-C:/Program Files (x86)/Windows Kits/10}"

# --- 取本机最新的工具集 / SDK 版本 ---
MSVC_VER="${MSVC_VER:-$(ls "$MSVC_BASE/VC/Tools/MSVC" 2>/dev/null | sort -V | tail -1)}"
SDK_VER="${SDK_VER:-$(ls "$SDK_ROOT/Include" 2>/dev/null | grep -E '^[0-9]+\.' | sort -V | tail -1)}"

if [ -z "$MSVC_VER" ] || [ -z "$SDK_VER" ]; then
    echo "[msvc-env] 未找到 MSVC 工具集或 Windows SDK，请检查 MSVC_BASE / SDK_ROOT" >&2
    return 1 2>/dev/null || exit 1
fi

# INCLUDE / LIB 要 Windows 形式（C:\...），PATH 要 POSIX 形式（/c/...）——
# 直接把 C:/... 塞进 PATH 会被冒号截断成 "C" 一段，cl.exe 就找不到了。
towin()   { cygpath -w "$1" 2>/dev/null || printf '%s' "$1"; }
toposix() { cygpath -u "$1" 2>/dev/null || printf '%s' "$1"; }

MSVC_INC="$(towin "$MSVC_BASE/VC/Tools/MSVC/$MSVC_VER/include")"
MSVC_LIB="$(towin "$MSVC_BASE/VC/Tools/MSVC/$MSVC_VER/lib/x64")"
MSVC_BIN="$(toposix "$MSVC_BASE/VC/Tools/MSVC/$MSVC_VER/bin/Hostx64/x64")"
SDK_INC="$(towin "$SDK_ROOT/Include/$SDK_VER")"
SDK_LIB="$(towin "$SDK_ROOT/Lib/$SDK_VER")"
SDK_BIN="$(toposix "$SDK_ROOT/bin/$SDK_VER/x64")"

export INCLUDE="$MSVC_INC;$SDK_INC\\ucrt;$SDK_INC\\um;$SDK_INC\\shared;$SDK_INC\\winrt;$SDK_INC\\cppwinrt"
export LIB="$MSVC_LIB;$SDK_LIB\\ucrt\\x64;$SDK_LIB\\um\\x64"
export PATH="$MSVC_BIN:$SDK_BIN:$PATH"

if [ "${MSVC_QUIET:-0}" != "1" ]; then
    if command -v cl >/dev/null 2>&1; then
        echo "MSVC 环境已加载: $(cl 2>&1 | head -1)"
    else
        echo "MSVC 环境已加载（INCLUDE/LIB 已导出，但 cl 不在 PATH 中）"
    fi
    echo "  MSVC $MSVC_VER  |  SDK $SDK_VER"
    echo "  $MSVC_BASE"
fi
