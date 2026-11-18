#!/usr/bin/env bash
# 编译 livecap，并放到 Tauri 要求的位置：src-tauri/binaries/livecap-<target-triple>
# 打包 macOS 版时由 tauri.macos.conf.json 的 beforeBuildCommand 自动调用
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/apps/desktop/src-tauri/binaries"
triple="${TAURI_ENV_TARGET_TRIPLE:-$(rustc -vV | sed -n 's/^host: //p')}"
mkdir -p "$out"
cd "$root/apps/livecap"
case "$triple" in
  universal-apple-darwin)
    swift build -c release --arch arm64 --arch x86_64
    src="$(swift build -c release --arch arm64 --arch x86_64 --show-bin-path)/livecap" ;;
  x86_64-apple-darwin)
    swift build -c release --arch x86_64
    src="$(swift build -c release --arch x86_64 --show-bin-path)/livecap" ;;
  *)
    swift build -c release --arch arm64
    src="$(swift build -c release --arch arm64 --show-bin-path)/livecap" ;;
esac
cp "$src" "$out/livecap-$triple"
echo "livecap -> $out/livecap-$triple"
