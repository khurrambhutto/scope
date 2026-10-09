#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: $0 <version> <output-directory> <linuxdeploy-appimage>" >&2
  exit 2
fi

version="$1"
output_dir="$2"
linuxdeploy_appimage="$3"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
binary="$repo_root/target/release/scope-gpui"
desktop_file="$repo_root/assets/scope.desktop"
icon_file="$repo_root/assets/scope.png"

if [[ ! "$version" =~ ^[0-9]+(\.[0-9]+){2}$ ]]; then
  echo "Expected a semantic version like 0.3.3, got: $version" >&2
  exit 2
fi
for input in "$binary" "$desktop_file" "$icon_file" "$linuxdeploy_appimage"; do
  if [[ ! -f "$input" ]]; then
    echo "Required AppImage input is missing: $input" >&2
    exit 1
  fi
done
if [[ ! -x "$linuxdeploy_appimage" ]]; then
  echo "linuxdeploy is not executable: $linuxdeploy_appimage" >&2
  exit 1
fi

mkdir -p "$output_dir"
output_dir="$(cd "$output_dir" && pwd)"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT
appdir="$work_dir/Scope.AppDir"
output="$output_dir/Scope-${version}-x86_64.AppImage"
mkdir -p "$appdir"

# The AppImage catalog requires this icon at the AppDir root.
cp "$icon_file" "$appdir/.DirIcon"

(
  cd "$work_dir"
  LDAI_OUTPUT="$output" APPIMAGE_EXTRACT_AND_RUN=1 "$linuxdeploy_appimage" \
    --appdir "$appdir" \
    --executable "$binary" \
    --desktop-file "$desktop_file" \
    --icon-file "$icon_file" \
    --output appimage
)

if [[ ! -s "$output" ]]; then
  echo "linuxdeploy did not create the expected AppImage: $output" >&2
  exit 1
fi

chmod 755 "$output"
echo "$output"
