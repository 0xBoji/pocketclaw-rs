#!/bin/bash
set -e

echo "🦞 Building PhoneClaw for Android..."

# Check requirements
if ! command -v cargo-ndk &> /dev/null; then
    echo "❌ cargo-ndk not found. Installing..."
    cargo install cargo-ndk
fi

# Ensure targets are added
echo "👉 Adding Rust targets..."
rustup target add aarch64-linux-android
rustup target add armv7-linuxandroideabi
rustup target add x86_64-linux-android

# Build Rust libraries manually first (optional, verification step)
echo "👉 Verifying Rust build..."
cargo ndk -t aarch64-linux-android -t armv7-linuxandroideabi -t x86_64-linux-android -o android/app/src/main/jniLibs build --release -p mobile-jni

echo "✅ Rust build complete. Native libs are in android/app/src/main/jniLibs/"
echo ""
echo "🚀 Now open 'android/' folder in Android Studio and build the APK!"
echo "   Or run: cd android && ./gradlew assembleDebug"
