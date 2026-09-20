#!/bin/bash
#=============================================================
# Tiny Reader - 发布脚本
# 用法:
#   ./release.sh v0.1.1        # 推送 main 并打标签，触发 GitHub Actions 构建
#   ./release.sh               # 自动取当前最新版本，补丁号 +1 后发布
#                              # (最新为 v0.1.6 时等同于 ./release.sh v0.1.7)
#
# 做的事:
#   1. 校验版本号格式 (vX.Y.Z)；未给版本号时自动推导下一个补丁版本
#   2. 前置检查：工作区干净、github 远程存在
#   3. 本地同步版本号到 src-tauri/tauri.conf.json、package.json、server/Cargo.toml
#      （本仓库未内置 CI 版本注入，需在打标签前把版本号落盘并提交）
#   4. 推送 main 到 github 远程
#   5. 若标签已存在则先删除（本地+远端），再创建并推送 vX.Y.Z 标签 (触发 Build workflow)
#=============================================================

set -euo pipefail

# ---- 参数校验 ----
if [ $# -gt 1 ]; then
  echo "用法: $0 [vX.Y.Z]   (例: $0 v0.1.1；不带参数则自动 +1 补丁版本)"
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

# ---- 前置检查 ----
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "ERROR: 工作区有未提交的改动，请先处理："
  git status --short
  exit 1
fi

REMOTE="github"
if ! git remote get-url "$REMOTE" >/dev/null 2>&1; then
  echo "ERROR: 未找到名为 '$REMOTE' 的远程，请检查 git remote -v"
  exit 1
fi

if [ $# -eq 1 ]; then
  TAG="$1"
  if [[ ! "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "ERROR: 版本号格式应为 vX.Y.Z (例: v0.1.1)，收到: $TAG"
    exit 1
  fi
else
  # 不带参数：取本地 + 远端标签里最大的 vX.Y.Z，补丁号 +1
  LATEST="$(
    {
      git tag -l 'v[0-9]*.[0-9]*.[0-9]*'
      git ls-remote --tags "$REMOTE" 'refs/tags/v[0-9]*.[0-9]*.[0-9]*' 2>/dev/null \
        | sed -e 's#.*refs/tags/##' -e '/\^{}$/d'
    } | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | sort -V | tail -1
  )"
  if [ -z "$LATEST" ]; then
    echo "ERROR: 未找到任何 vX.Y.Z 标签，请显式指定版本号：$0 v0.1.0"
    exit 1
  fi
  IFS='.' read -r MAJOR MINOR PATCH <<<"${LATEST#v}"
  TAG="v${MAJOR}.${MINOR}.$((PATCH + 1))"
  echo "==> 当前最新版本 $LATEST，自动发布下一版本 $TAG"
fi

VERSION="${TAG#v}"
echo "==> 发布 $TAG (版本号 $VERSION)"

# ---- 同步版本号到本地配置文件 ----
# 本仓库没有 CI 自动注入版本的机制，版本号必须体现在产物里，
# 因此在打标签前先把版本写进三个配置并生成一个提交。
echo "==> 同步版本号 $VERSION 到本地配置"

sync_version() {
  local file="$1"
  if [ ! -f "$file" ]; then
    echo "    跳过（文件不存在）: $file"
    return 0
  fi
  if command -v sed >/dev/null 2>&1; then
    case "$file" in
      */tauri.conf.json|*/package.json)
        # 只替换 "version" 字段，避免误伤 dependencies 里的 ^x.y.z
        sed -i.bak -E "s/(\"version\"[[:space:]]*:[[:space:]]*\")[0-9]+\.[0-9]+\.[0-9]+(\")/\1${VERSION}\2/" "$file"
        ;;
      */Cargo.toml)
        # 只替换 [package] 段下的 version（取第一次出现）
        sed -i.bak -E "0,/^version[[:space:]]*=/s/^(version[[:space:]]*=[[:space:]]*\")[0-9]+\.[0-9]+\.[0-9]+(\")/\1${VERSION}\2/" "$file"
        ;;
    esac
    rm -f "${file}.bak"
    echo "    已更新: $file"
  else
    echo "ERROR: 未找到 sed，无法同步版本号"
    exit 1
  fi
}

sync_version "src-tauri/tauri.conf.json"
sync_version "package.json"
sync_version "server/Cargo.toml"

# 若版本号确实发生变化则提交
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "==> 提交版本号变更"
  git add src-tauri/tauri.conf.json package.json server/Cargo.toml
  git commit -m "chore: bump version to $VERSION"
else
  echo "==> 版本号无变化，无需提交"
fi

echo "==> 推送 main 到 $REMOTE"
git push "$REMOTE" main

echo "==> 创建并推送标签 $TAG"
# 若标签已存在（本地或远端）则先删除，便于重发同一版本。
# 注意：删除 git 标签不会删除已关联的 GitHub Release。
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "    本地标签 $TAG 已存在，删除"
  git tag -d "$TAG"
fi
if git ls-remote --tags "$REMOTE" "refs/tags/$TAG" | grep -q .; then
  echo "    远端标签 $TAG 已存在，删除"
  git push "$REMOTE" --delete "$TAG"
fi
git tag "$TAG"
git push "$REMOTE" "$TAG"

echo ""
echo "✅ 完成。GitHub Actions Build workflow 已由标签 $TAG 触发。"
echo "   https://github.com/simonlei/tiny-reader/actions"
echo "   产物: 服务端 linux-x86_64、客户端 windows-x86_64 / macos-universal"
