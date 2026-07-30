#!/bin/sh
set -eu

if [ "$#" -ne 2 ] || { [ "$2" != "local" ] && [ "$2" != "bundle" ]; }; then
    echo "usage: $0 ARCHIVE local|bundle" >&2
    exit 2
fi

archive=$1
mode=$2
if [ ! -f "$archive" ]; then
    echo "plugin archive does not exist: $archive" >&2
    exit 1
fi

listing=$(mktemp)
trap 'rm -f "$listing"' EXIT HUP INT TERM
unzip -Z1 "$archive" >"$listing"

require_entry() {
    if ! grep -Fxq "$1" "$listing"; then
        echo "plugin archive is missing: $1" >&2
        exit 1
    fi
}

require_entry "addons/themosis/plugin.cfg"
require_entry "addons/themosis/plugin.gd"
require_entry "addons/themosis/plugin.gd.uid"
require_entry "addons/themosis/profile_store.gd"
require_entry "addons/themosis/profile_store.gd.uid"
require_entry "addons/themosis/theme_dock.gd"
require_entry "addons/themosis/theme_dock.gd.uid"
require_entry "addons/themosis/theme_importer.gd"
require_entry "addons/themosis/theme_importer.gd.uid"
require_entry "addons/themosis/theme_builder.gd"
require_entry "addons/themosis/theme_builder.gd.uid"
require_entry "addons/themosis/build.gd"
require_entry "addons/themosis/build.gd.uid"
require_entry "addons/themosis/README.md"
require_entry "addons/themosis/LICENSE"
require_entry "addons/themosis/themosis.gdextension"
require_entry "addons/themosis/docs/TOKEN_FORMAT.md"
require_entry "addons/themosis/docs/KDL_FORMAT.md"
require_entry "addons/themosis/docs/GODOT_MAPPINGS.md"

if [ "$mode" = "bundle" ]; then
    require_entry "addons/themosis/bin/linux/x86_64/libthemosis_godot.so"
    require_entry "addons/themosis/bin/windows/x86_64/themosis_godot.dll"
    require_entry "addons/themosis/bin/macos/x86_64/libthemosis_godot.dylib"
    require_entry "addons/themosis/bin/macos/arm64/libthemosis_godot.dylib"
elif ! grep -Eq '^addons/themosis/bin/(linux|macos|windows)/(x86_64|arm64)/[^/]+$' "$listing"; then
    echo "local plugin archive contains no supported native library" >&2
    exit 1
fi

echo "verified $archive ($mode)"
