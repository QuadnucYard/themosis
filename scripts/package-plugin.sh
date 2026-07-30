#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
source_addon="$repository_root/examples/godot/addons/themosis"

version=$(awk '
    /^\[workspace\.package\]$/ { in_package = 1; next }
    /^\[/ { if (in_package) exit }
    in_package && /^version[[:space:]]*=/ {
        value = $0
        sub(/^[^=]*=[[:space:]]*"/, "", value)
        sub(/"[[:space:]]*$/, "", value)
        print value
        exit
    }
' "$repository_root/Cargo.toml")
if [ -z "$version" ]; then
    echo "cannot determine workspace version" >&2
    exit 1
fi

plugin_version=$(awk -F '"' '/^version=/ { print $2; exit }' "$source_addon/plugin.cfg")
if [ "$plugin_version" != "$version" ]; then
    echo "plugin version $plugin_version does not match workspace version $version" >&2
    exit 1
fi

usage() {
    echo "usage: $0 [--native-root DIRECTORY]" >&2
    echo "without arguments, builds a package for the current host only" >&2
    exit 2
}

native_root=""
if [ "$#" -eq 2 ] && [ "$1" = "--native-root" ]; then
    native_root=$2
elif [ "$#" -ne 0 ]; then
    usage
fi

normalize_architecture() {
    case "$1" in
        x86_64|amd64|AMD64)
            echo "x86_64"
            ;;
        arm64|aarch64)
            echo "arm64"
            ;;
        *)
            echo "unsupported packaging architecture: $1" >&2
            exit 1
            ;;
    esac
}

library_name() {
    case "$1" in
        linux)
            echo "libthemosis_godot.so"
            ;;
        macos)
            echo "libthemosis_godot.dylib"
            ;;
        windows)
            echo "themosis_godot.dll"
            ;;
        *)
            echo "unsupported packaging platform: $1" >&2
            exit 1
            ;;
    esac
}

copy_library() {
    source=$1
    platform=$2
    architecture=$3
    name=$(library_name "$platform")
    if [ ! -f "$source" ]; then
        echo "missing native library: $source" >&2
        exit 1
    fi
    destination="$addon/bin/$platform/$architecture"
    mkdir -p "$destination"
    cp "$source" "$destination/$name"
}

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT HUP INT TERM
addon="$stage/addons/themosis"
mkdir -p "$addon"

cp "$source_addon/plugin.cfg" "$addon/plugin.cfg"
cp "$source_addon/plugin.gd" "$addon/plugin.gd"
cp "$source_addon/plugin.gd.uid" "$addon/plugin.gd.uid"
cp "$source_addon/profile_store.gd" "$addon/profile_store.gd"
cp "$source_addon/profile_store.gd.uid" "$addon/profile_store.gd.uid"
cp "$source_addon/theme_dock.gd" "$addon/theme_dock.gd"
cp "$source_addon/theme_dock.gd.uid" "$addon/theme_dock.gd.uid"
cp "$source_addon/theme_importer.gd" "$addon/theme_importer.gd"
if [ -f "$source_addon/theme_importer.gd.uid" ]; then
    cp "$source_addon/theme_importer.gd.uid" "$addon/theme_importer.gd.uid"
fi
cp "$source_addon/theme_builder.gd" "$addon/theme_builder.gd"
cp "$source_addon/theme_builder.gd.uid" "$addon/theme_builder.gd.uid"
cp "$source_addon/build.gd" "$addon/build.gd"
cp "$source_addon/build.gd.uid" "$addon/build.gd.uid"
cp "$source_addon/README.md" "$addon/README.md"
cp "$repository_root/LICENSE" "$addon/LICENSE"
cp "$source_addon/themosis.gdextension.package" "$addon/themosis.gdextension"
mkdir -p "$addon/docs"
cp "$repository_root/crates/themosis-tokens/FORMAT.md" "$addon/docs/TOKEN_FORMAT.md"
cp "$repository_root/crates/themosis-kdl/FORMAT.md" "$addon/docs/KDL_FORMAT.md"
cp "$repository_root/crates/themosis-godot/MAPPINGS.md" "$addon/docs/GODOT_MAPPINGS.md"

mkdir -p "$repository_root/dist"
if [ -n "$native_root" ]; then
    copy_library "$native_root/linux/x86_64/libthemosis_godot.so" linux x86_64
    copy_library "$native_root/windows/x86_64/themosis_godot.dll" windows x86_64
    copy_library "$native_root/macos/x86_64/libthemosis_godot.dylib" macos x86_64
    copy_library "$native_root/macos/arm64/libthemosis_godot.dylib" macos arm64

    if [ -f "$native_root/linux/arm64/libthemosis_godot.so" ]; then
        copy_library "$native_root/linux/arm64/libthemosis_godot.so" linux arm64
    fi
    if [ -f "$native_root/windows/arm64/themosis_godot.dll" ]; then
        copy_library "$native_root/windows/arm64/themosis_godot.dll" windows arm64
    fi

    archive="$repository_root/dist/themosis-godot-$version.zip"
    verification=bundle
else
    case "$(uname -s)" in
        Darwin)
            platform=macos
            ;;
        Linux)
            platform=linux
            ;;
        MINGW*|MSYS*|CYGWIN*)
            platform=windows
            ;;
        *)
            echo "unsupported packaging platform: $(uname -s)" >&2
            exit 1
            ;;
    esac
    architecture=$(normalize_architecture "$(uname -m)")
    name=$(library_name "$platform")

    cargo build --manifest-path "$repository_root/Cargo.toml" -p themosis-godot-plugin --release
    copy_library "$repository_root/target/release/$name" "$platform" "$architecture"

    archive="$repository_root/dist/themosis-godot-$version-$platform-$architecture.zip"
    verification=local
fi

rm -f "$archive"
(cd "$stage" && zip -q -r "$archive" addons)
"$repository_root/scripts/verify-plugin-package.sh" "$archive" "$verification"
echo "created $archive"
