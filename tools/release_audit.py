#!/usr/bin/env python3
"""Fail-closed source audit used locally and by release CI."""

from __future__ import annotations

import json
import re
import sys
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    raise SystemExit(f"release audit failed: {message}")


def main() -> None:
    package = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]
    native = tomllib.loads((ROOT / "android/native/Cargo.toml").read_text())["package"]
    version = package["version"]
    if native["version"] != version:
        fail(f"Rust package versions differ: {version} vs {native['version']}")

    gradle = (ROOT / "android/app/build.gradle.kts").read_text()
    version_name = re.search(r'versionName\s*=\s*"([^"]+)"', gradle)
    version_code = re.search(r"versionCode\s*=\s*(\d+)", gradle)
    if not version_name or version_name.group(1) != version:
        fail("Android versionName does not match Cargo package")
    if not version_code:
        fail("Android versionCode is missing")
    for workflow_name in ("ci.yml", "android.yml"):
        workflow = (ROOT / ".github/workflows" / workflow_name).read_text()
        if f'NEXUS_VERSION: "{version}"' not in workflow:
            fail(f"{workflow_name} artifact version does not match Cargo package")

    for path in ROOT.rglob("*.toml"):
        tomllib.loads(path.read_text())
    for path in ROOT.rglob("*.xml"):
        ET.parse(path)

    kotlin = "\n".join(path.read_text() for path in ROOT.rglob("*.kt"))
    native_rust = "\n".join(path.read_text() for path in (ROOT / "android/native").rglob("*.rs"))
    declarations = set(re.findall(r"external\s+fun\s+(\w+)", kotlin))
    exports = set(re.findall(r"Java_ai_nexus_shell_NativeBridge_(\w+)", native_rust))
    if declarations != exports:
        fail(f"JNI mismatch: Kotlin-only={sorted(declarations - exports)}, Rust-only={sorted(exports - declarations)}")

    app_sources = "\n".join(path.read_text(errors="replace") for path in (ROOT / "android/app/src").rglob("*") if path.is_file())
    if re.search(r"android\.webkit|\bWebView\b", app_sources):
        fail("WebView dependency detected in Android application source")

    rust_files = list(ROOT.rglob("*.rs"))
    kotlin_files = list(ROOT.rglob("*.kt"))
    result = {
        "version": version,
        "version_code": int(version_code.group(1)),
        "rust_files": len(rust_files),
        "rust_lines": sum(len(path.read_text().splitlines()) for path in rust_files),
        "rust_tests": sum(len(re.findall(r"#\[(?:tokio::)?test\]", path.read_text())) for path in rust_files),
        "kotlin_files": len(kotlin_files),
        "kotlin_lines": sum(len(path.read_text().splitlines()) for path in kotlin_files),
        "jni_symbols": len(declarations),
        "webview_free": True,
    }
    output = json.dumps(result, indent=2, sort_keys=True)
    print(output)
    if len(sys.argv) == 3 and sys.argv[1] == "--output":
        Path(sys.argv[2]).write_text(output + "\n")


if __name__ == "__main__":
    main()
