#!/usr/bin/env nu
# Verifies that a packaged addon archive has the documented layout.
#
# usage: nu scripts/verify-plugin-package.nu ARCHIVE local|bundle

# Prints the usage message and exits.
def usage [] {
    print -e "usage: nu scripts/verify-plugin-package.nu ARCHIVE local|bundle"
    exit 2
}

# Fails unless the archive listing contains `entry`.
def require-entry [listing: list<string>, entry: string] {
    if $entry not-in $listing {
        print -e $"plugin archive is missing: ($entry)"
        exit 1
    }
}

# Verifies that ARCHIVE contains the layout documented for `local` or `bundle`
# archives.
def main [...args: string] {
    if ($args | length) != 2 {
        usage
    }
    let archive = $args.0
    let mode = $args.1
    if $mode not-in ["local" "bundle"] {
        usage
    }
    if not ($archive | path exists) {
        print -e $"plugin archive does not exist: ($archive)"
        exit 1
    }

    let listing = (^unzip -Z1 $archive | lines)
    for entry in [
        "addons/themosis/README.md"
        "addons/themosis/LICENSE"
        "addons/themosis/themosis.gdextension"
        "addons/themosis/docs/TOKEN_FORMAT.md"
        "addons/themosis/docs/KDL_FORMAT.md"
        "addons/themosis/docs/GODOT_MAPPINGS.md"
    ] {
        require-entry $listing $entry
    }

    if $mode == "bundle" {
        for entry in [
            "addons/themosis/bin/linux/x86_64/libthemosis_godot_plugin.so"
            "addons/themosis/bin/windows/x86_64/themosis_godot_plugin.dll"
            "addons/themosis/bin/macos/x86_64/libthemosis_godot_plugin.dylib"
            "addons/themosis/bin/macos/arm64/libthemosis_godot_plugin.dylib"
        ] {
            require-entry $listing $entry
        }
    } else if not (
        $listing | any {|entry|
            $entry =~ '^addons/themosis/bin/(linux|macos|windows)/(x86_64|arm64)/[^/]+$'
        }
    ) {
        print -e "local plugin archive contains no supported native library"
        exit 1
    }

    print ("verified " + $archive + " (" + $mode + ")")
}
