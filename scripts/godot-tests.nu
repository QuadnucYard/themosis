#!/usr/bin/env nu
# Runs the runtime-backed CLI and plugin suites against one GDExtension build.
#
# The library is built with the `test-support` probes and its path is taken
# from Cargo's own build output, so a redirected `CARGO_TARGET_DIR` or a
# non-default profile cannot make a suite load a different artifact. The path
# and the engine are then pinned through `THEMOSIS_GODOT_LIBRARY` and
# `THEMOSIS_GODOT_BINARY` for every suite and for the final import gate.
#
# Missing prerequisites fail instead of skipping, and the gallery import gate
# runs last as the export validation documented in `examples/godot/README.md`.

# The filename Cargo builds the plugin library as on this platform.
def library-name [] {
    match $nu.os-info.name {
        "macos" => { "libthemosis_godot_plugin.dylib" }
        "linux" => { "libthemosis_godot_plugin.so" }
        "windows" => { "themosis_godot_plugin.dll" }
        _ => {
            print -e "unsupported platform for the Godot test suites"
            exit 1
        }
    }
}

# Returns the Godot executable, honoring `THEMOSIS_GODOT_BINARY` first and
# falling back to `godot` or `godot4` on `PATH`.
def find-godot [] {
    let pinned = ($env.THEMOSIS_GODOT_BINARY? | default "")
    if $pinned != "" {
        return $pinned
    }
    for candidate in ["godot" "godot4"] {
        let found = (which $candidate)
        if ($found | is-not-empty) {
            return ($found | get 0.path | into string)
        }
    }
    print -e "Godot was not found; install Godot 4.5+ or set THEMOSIS_GODOT_BINARY"
    exit 1
}

# Builds the plugin with the `test-support` probes and returns the artifact
# Cargo reported, so a suite cannot load a library other than this run's build.
def build-library [name: string] {
    let built = (
        do {
            ^cargo build -p themosis-godot-plugin --features test-support --message-format=json
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

# Writes a `.gdextension` manifest that loads the pinned library for every
# platform and architecture key the suites run on.
def write-extension-manifest [project: path, library: string] {
    let keys = [
        "macos.debug"
        "macos.debug.arm64"
        "macos.debug.x86_64"
        "linux.debug.x86_64"
        "linux.debug.arm64"
        "windows.debug.x86_64"
        "windows.debug.arm64"
    ]
    let manifest = (
        [
            "[configuration]"
            'entry_symbol = "gdext_rust_init"'
            "compatibility_minimum = 4.5"
            "reloadable = true"
            ""
            "[libraries]"
        ]
        | append ($keys | each {|key| $'($key) = "($library)"' })
        | str join (char nl)
    )
    $manifest | save -f ($project | path join "themosis.gdextension")
}

# The import gate runs on a staged copy of the gallery so it validates the same
# pinned artifact as the suites, instead of the workspace path in the example's
# developer manifest. The copy also leaves the checked-in project untouched.
def run-import-gate [godot: string, library: string, repository_root: path] {
    let stage = (^mktemp -d | str trim)
    try {
        cp -r ($repository_root | path join "examples" "godot") $stage
        let project = ($stage | path join "godot")
        rm -rf ($project | path join ".godot") ($project | path join ".themosis") ($project | path join "addons")
        write-extension-manifest $project $library

        # Load the extension at startup, before the first filesystem scan.
        # Discovering it mid-scan queues editor documentation callbacks that
        # can run after their data is freed at headless import shutdown:
        # https://github.com/godotengine/godot/issues/111645
        mkdir ($project | path join ".godot")
        "res://themosis.gdextension\n" | save -f ($project | path join ".godot" "extension_list.cfg")

        # The exit status of `--import` is not a reliable signal: a project
        # without an import cache reports errors for scenes that reference the
        # themes the same pass imports, and stale imports still exit zero. A
        # staged copy starts cold, so it needs the same second pass the gallery
        # suite performs; the gate below is the authoritative validation, as
        # documented in examples/godot/README.md.
        for _ in [1 2] {
            do { ^$godot --headless --editor --path $project --import } | complete | ignore
        }
        # Keep the runner as a statement so Nu waits before cleanup. A final
        # external pipeline races `finally` on older Nu versions.
        ^$godot --headless --path $project --main-loop ThemosisCheckRunner
        null
    } finally {
        rm -rf $stage
    }
}

# Builds the pinned library and runs the runtime-backed CLI and plugin suites,
# ending with the gallery import gate.
def main [] {
    let repository_root = ($env.FILE_PWD | path dirname)
    let godot = (find-godot)
    let library = (build-library (library-name))

    $env.THEMOSIS_REQUIRE_GODOT = "1"
    $env.THEMOSIS_GODOT_LIBRARY = $library
    $env.THEMOSIS_GODOT_BINARY = $godot

    ^cargo test -p themosis-cli --test check --test godot
    ^cargo test -p themosis-godot-plugin --features test-support
    run-import-gate $godot $library $repository_root
}
