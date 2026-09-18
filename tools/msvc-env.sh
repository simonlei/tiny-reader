# 在 Git Bash 里手动搭 MSVC 命令行环境（cmd.exe / vcvarsall.bat 不可用时的替代方案）
#
# 用法：  source tools/msvc-env.sh
#
# 只影响当前 shell，不会污染系统。

MSVC_ROOT="C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/VC/Tools/MSVC/14.44.35207"
SDK_ROOT="C:/Program Files (x86)/Windows Kits/10"
SDK_VER="10.0.26100.0"

export INCLUDE="C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Tools\\MSVC\\14.44.35207\\include"
export INCLUDE="$INCLUDE;C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$SDK_VER\\ucrt"
export INCLUDE="$INCLUDE;C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$SDK_VER\\um"
export INCLUDE="$INCLUDE;C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$SDK_VER\\shared"
export INCLUDE="$INCLUDE;C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$SDK_VER\\winrt"
export INCLUDE="$INCLUDE;C:\\Program Files (x86)\\Windows Kits\\10\\Include\\$SDK_VER\\cppwinrt"

export LIB="C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Tools\\MSVC\\14.44.35207\\lib\\x64"
export LIB="$LIB;C:\\Program Files (x86)\\Windows Kits\\10\\Lib\\$SDK_VER\\ucrt\\x64"
export LIB="$LIB;C:\\Program Files (x86)\\Windows Kits\\10\\Lib\\$SDK_VER\\um\\x64"

export PATH="$MSVC_ROOT/bin/Hostx64/x64:$SDK_ROOT/bin/$SDK_VER/x64:$PATH"

echo "MSVC 环境已加载: $(cl 2>&1 | head -1)"
