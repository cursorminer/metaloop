#!/bin/bash
# Run with ./bmv.sh (not `source`) to build and install the plugin.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VST3_DEST="/Library/Audio/Plug-Ins/VST3"
BUNDLE_SRC="$SCRIPT_DIR/target/bundled/Metaloop.vst3"
BUNDLE_DEST="$VST3_DEST/Metaloop.vst3"

if ! cargo xtask bundle metaloop --release; then
    echo "Build failed, not installing." >&2
    return 1 2>/dev/null || exit 1
fi

if [ ! -d "$BUNDLE_SRC" ]; then
    echo "Bundle not found at $BUNDLE_SRC" >&2
    return 1 2>/dev/null || exit 1
fi

mkdir -p "$VST3_DEST"
rm -rf "$BUNDLE_DEST"
mv "$BUNDLE_SRC" "$BUNDLE_DEST"

echo "Installed Metaloop.vst3 to $BUNDLE_DEST"
