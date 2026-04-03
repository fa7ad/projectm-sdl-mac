#!/bin/bash

# Exit on error
set -e

APP_NAME="mProject"
BINARY_NAME="projectm_sdl"
BUNDLE_DIR="target/macos/${APP_NAME}.app"
CONTENTS_DIR="${BUNDLE_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

MACOS_ASSETS="res/macos"

echo "==> Building ${BINARY_NAME}..."
cargo build --release

echo "==> Recreating app bundle structure..."
rm -rf "${BUNDLE_DIR}"
mkdir -p "${MACOS_DIR}"
mkdir -p "${RESOURCES_DIR}"

# Copy binary
cp "target/release/${BINARY_NAME}" "${MACOS_DIR}/${BINARY_NAME}"

# Copy wrapper script
if [ -f "${MACOS_ASSETS}/run" ]; then
    cp "${MACOS_ASSETS}/run" "${MACOS_DIR}/run"
    chmod +x "${MACOS_DIR}/run"
else
    echo "Warning: Wrapper script 'run' not found in ${MACOS_ASSETS}/"
    cat > "${MACOS_DIR}/run" <<EOL
#!/usr/bin/env bash
cd \$(dirname "\$0")/../..
./Contents/MacOS/${BINARY_NAME}
EOL
    chmod +x "${MACOS_DIR}/run"
fi

# Copy Info.plist
if [ -f "${MACOS_ASSETS}/Info.plist" ]; then
    cp "${MACOS_ASSETS}/Info.plist" "${CONTENTS_DIR}/Info.plist"
else
    echo "Warning: Info.plist not found in ${MACOS_ASSETS}/"
fi

# Copy icons and config
if [ -f "${MACOS_ASSETS}/icon.icns" ]; then
    cp "${MACOS_ASSETS}/icon.icns" "${RESOURCES_DIR}/icon.icns"
fi

if [ -f "${MACOS_ASSETS}/config.toml" ]; then
    cp "${MACOS_ASSETS}/config.toml" "${RESOURCES_DIR}/config.toml"
fi

# Copy presets and textures from res/
echo "==> Syncing presets and textures..."
mkdir -p "${RESOURCES_DIR}/presets"
mkdir -p "${RESOURCES_DIR}/textures"

# Use rsync to exclude .git for a cleaner bundle
if command -v rsync >/dev/null 2>&1; then
    rsync -a --exclude=".git" "res/presets-cream-of-the-crop/" "${RESOURCES_DIR}/presets/"
    rsync -a --exclude=".git" "res/presets-milkdrop-texture-pack/textures/" "${RESOURCES_DIR}/textures/"
else
    cp -R "res/presets-cream-of-the-crop/"* "${RESOURCES_DIR}/presets/"
    cp -R "res/presets-milkdrop-texture-pack/textures/"* "${RESOURCES_DIR}/textures/"
    find "${RESOURCES_DIR}" -name ".git" -type d -exec rm -rf {} + 2>/dev/null || true
fi

echo "✅ Build complete: ${BUNDLE_DIR}"
echo "You can run it with: open ${BUNDLE_DIR}"
