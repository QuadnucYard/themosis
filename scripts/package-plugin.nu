#!/usr/bin/env nu
# Builds and verifies the Godot addon package.
#
#   nu scripts/package-plugin.nu
#       builds the library for the current host and packages it
#   nu scripts/package-plugin.nu --native-root DIRECTORY
#       assembles a cross-platform package from
#       `DIRECTORY/<platform>/<architecture>/<library>`

# Prints the usage message and exits.
def usage [] {
    print -e "usage: nu scripts/package-plugin.nu [--native-root DIRECTORY]"
    print -e "without arguments, builds a package for the current host only"
    exit 2
}

# The library filename for one packaging platform.
def library-name [platform: string] {
    match $platform {
        "linux" => { "libthemosis_godot_plugin.so" }
        "macos" => { "libthemosis_godot_plugin.dylib" }
        "windows" => { "themosis_godot_plugin.dll" }
        _ => {
            print -e $"unsupported packaging platform: ($platform)"
            exit 1
        }
    }
}

# Maps architecture spellings seen across hosts and CI onto layout names.
def normalize-architecture [architecture: string] {
    match $architecture {
        "x86_64" | "amd64" | "AMD64" => { "x86_64" }
        "arm64" | "aarch64" => { "arm64" }
        _ => {
            print -e $"unsupported packaging architecture: ($architecture)"
            exit 1
        }
    }
}

# The packaging platform name of the running host.
def host-platform [] {
    match $nu.os-info.name {
        "linux" => { "linux" }
        "macos" => { "macos" }
        "windows" => { "windows" }
        _ => {
            print -e $"unsupported packaging platform: ($nu.os-info.name)"
            exit 1
        }
    }
}

# Copies one library into the addon's `bin/<platform>/<architecture>` layout.
def copy-library [addon: path, source: path, platform: string, architecture: string] {
    if not ($source | path exists) {
        error make {msg: $"missing native library: ($source)"}
    }
    let name = (library-name $platform)
    let destination = ($addon | path join "bin" $platform $architecture)
    mkdir $destination
    cp $source ($destination | path join $name)
}

# Builds the current host's GDExtension library and returns the artifact Cargo
# reported. Reading the path from Cargo's own output keeps a redirected
# `CARGO_TARGET_DIR` or a non-default profile from shipping a stale library.
def build-local-library [repository_root: path, platform: string] {
    let name = (library-name $platform)
    let built = (
        do {
            ^cargo build --manifest-path ($repository_root | path join "Cargo.toml") -p themosis-godot-plugin --release --message-format=json
        } | complete
    )
    if ($built.stderr | is-not-empty) {
        print -e ($built.stderr | str trim)
    }
    if $built.exit_code != 0 {
        print -e "the GDExtension library failed to build"
        exit 1
    }
    let library = (
        $built.stdout
        | lines
        | each {|line| $line | from json }
        | where {|artifact| $artifact.reason? == "compiler-artifact" }
        | get filenames
        | flatten
        | where {|file| ($file | path basename) == $name }
        | get 0?
    )
    if ($library | is-empty) {
        print -e $"Cargo did not report the GDExtension library \(($name)\)"
        exit 1
    }
    if not ($library | path exists) {
        print -e $"the GDExtension library does not exist: ($library)"
        exit 1
    }
    $library
}

# Copies the addon guide, license, extension template, and source contracts
# into the staged addon.
def stage-addon-assets [addon: path, repository_root: path] {
    mkdir ($addon | path join "docs")
    cp ($repository_root | path join "crates/themosis-godot-plugin/addon/README.md") ($addon | path join "README.md")
    cp ($repository_root | path join "LICENSE") ($addon | path join "LICENSE")
    cp ($repository_root | path join "crates/themosis-godot-plugin/addon/themosis.gdextension.in") ($addon | path join "themosis.gdextension")
    cp ($repository_root | path join "crates/themosis-tokens/FORMAT.md") ($addon | path join "docs/TOKEN_FORMAT.md")
    cp ($repository_root | path join "crates/themosis-kdl/FORMAT.md") ($addon | path join "docs/KDL_FORMAT.md")
    cp ($repository_root | path join "crates/themosis-godot/MAPPINGS.md") ($addon | path join "docs/GODOT_MAPPINGS.md")
}

# Zips the staged addon and runs `verify-plugin-package.nu` before reporting
# success.
def finish [stage: path, archive: path, mode: string, repository_root: path] {
    rm -f $archive
    let zipped = (do { cd $stage; ^zip -q -r $archive addons } | complete)
    if $zipped.exit_code != 0 {
        error make {msg: $"zip failed: ($zipped.stderr | str trim)"}
    }
    let verify = ($repository_root | path join "scripts" "verify-plugin-package.nu")
    let verified = (do { ^($nu.current-exe) $verify $archive $mode } | complete)
    if ($verified.stdout | is-not-empty) {
        print ($verified.stdout | str trim)
    }
    if $verified.exit_code != 0 {
        if ($verified.stderr | is-not-empty) {
            print -e ($verified.stderr | str trim)
        }
        exit $verified.exit_code
    }
    print $"created ($archive)"
}

# Builds the host package, or assembles a bundle from `--native-root`, and
# writes the verified archive under `dist/`.
def main [--native-root: string] {
    let repository_root = ($env.FILE_PWD | path dirname)
    let version = (
        open ($repository_root | path join "Cargo.toml")
        | get workspace.package.version
    )
    let stage = (^mktemp -d | str trim)
    try {
        let addon = ($stage | path join "addons" "themosis")
        mkdir $addon
        stage-addon-assets $addon $repository_root

        let dist = ($repository_root | path join "dist")
        mkdir $dist

        if ($native_root | default "") != "" {
            copy-library $addon ($native_root | path join "linux/x86_64/libthemosis_godot_plugin.so") "linux" "x86_64"
            copy-library $addon ($native_root | path join "windows/x86_64/themosis_godot_plugin.dll") "windows" "x86_64"
            copy-library $addon ($native_root | path join "macos/x86_64/libthemosis_godot_plugin.dylib") "macos" "x86_64"
            copy-library $addon ($native_root | path join "macos/arm64/libthemosis_godot_plugin.dylib") "macos" "arm64"

            let linux_arm64 = ($native_root | path join "linux/arm64/libthemosis_godot_plugin.so")
            if ($linux_arm64 | path exists) {
                copy-library $addon $linux_arm64 "linux" "arm64"
            }
            let windows_arm64 = ($native_root | path join "windows/arm64/themosis_godot_plugin.dll")
            if ($windows_arm64 | path exists) {
                copy-library $addon $windows_arm64 "windows" "arm64"
            }

            finish $stage ($dist | path join $"themosis-godot-($version).zip") "bundle" $repository_root
        } else {
            let platform = (host-platform)
            let architecture = (normalize-architecture $nu.os-info.arch)
            let library = (build-local-library $repository_root $platform)
            copy-library $addon $library $platform $architecture

            let archive = ($dist | path join $"themosis-godot-($version)-($platform)-($architecture).zip")
            finish $stage $archive "local" $repository_root
        }
    } catch {|error|
        print -e $"packaging failed: ($error.msg? | default ($error | to text))"
        exit 1
    } finally {
        rm -rf $stage
    }
}
