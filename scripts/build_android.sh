#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

for command_name in python3 cargo cargo-ndk gradle; do
    command -v "$command_name" >/dev/null || { echo "missing required command: $command_name" >&2; exit 1; }
done
: "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME must point to an installed Android NDK}"

python3 tools/release_audit.py --output release-audit.json
cargo test --all-targets
cargo build --release

mkdir -p android/app/src/main/jniLibs
(
    cd android/native
    cargo ndk --target arm64-v8a --platform 26 --output-dir ../app/src/main/jniLibs build --release
)
test -s android/app/src/main/jniLibs/arm64-v8a/libnexus_android.so
(
    cd android
    gradle --no-daemon :app:assembleDebug
)
apk="android/app/build/outputs/apk/debug/app-debug.apk"
test -s "$apk"
sha256sum "$apk"
