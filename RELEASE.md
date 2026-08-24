# Nexus 1.02 build and APK release procedure

## Required toolchain

- Rust 1.88.0 with `aarch64-linux-android`
- Java 17
- Android SDK 36 and Build Tools 36.0.0
- Android NDK 28.2.13676358
- Gradle 9.1.0
- `cargo-ndk`

## Automated build

The `Nexus Android Shell` workflow performs the source audit, compiles the
ARM64 JNI library, builds the debug APK, hashes it and publishes all three
outputs as one GitHub Actions artifact.

Expected files:

- `app-debug.apk`
- `nexus-android-sha256.txt`
- `release-audit.json`

## Local build

With the required tools installed and `ANDROID_NDK_HOME` configured:

```text
./scripts/build_android.sh
```

The installable debug APK is created at:

```text
android/app/build/outputs/apk/debug/app-debug.apk
```

Install it on a connected Android device with:

```text
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

An APK is not considered released until the workflow finishes successfully,
the SHA-256 file matches and an install/launch smoke test passes on a device.
