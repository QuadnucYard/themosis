#!/usr/bin/env nu
# Regression coverage for `scripts/package-plugin.nu`.
#
# The packaged library must be the artifact Cargo reports. A stale file left
# under the default `target/` directory must not change what ships when
# `CARGO_TARGET_DIR` is redirected, and a build whose Cargo output never
# reports the library must fail instead of packaging something else.

# The library filename for the host platform.
def library-name [] {
    match $nu.os-info.name {
        "linux" => { "libthemosis_godot_plugin.so" }
        "macos" => { "libthemosis_godot_plugin.dylib" }
        "windows" => { "themosis_godot_plugin.dll" }
        _ => {
            print -e $"unsupported platform for the packaging regression: ($nu.os-info.name)"
            exit 1
        }
    }
}

# Fails unless `unzip` is on `PATH`; archive inspection needs it.
def require-unzip [] {
    let found = (which unzip)
    if ($found | is-empty) {
        print -e "unzip is required to inspect the packaged archive"
        exit 1
    }
}

# Stages the files `package-plugin.nu` reads from the repository root.
def stage-workspace [repository_root: path, stage: path] {
    for relative in [
        "scripts/package-plugin.nu"
        "scripts/verify-plugin-package.nu"
        "crates/themosis-godot-plugin/addon/README.md"
        "crates/themosis-godot-plugin/addon/themosis.gdextension.in"
        "crates/themosis-tokens/FORMAT.md"
        "crates/themosis-kdl/FORMAT.md"
        "crates/themosis-godot/MAPPINGS.md"
        "Cargo.toml"
        "LICENSE"
    ] {
        let destination = ($stage | path join $relative)
        mkdir ($destination | path dirname)
        cp ($repository_root | path join $relative) $destination
    }
}

# Writes a fake `cargo` that "builds" the library into `CARGO_TARGET_DIR` and
# reports it like Cargo does, or hides it from its report entirely.
def write-fake-cargo [bin: path, name: string, reported: bool] {
    mkdir $bin
    let lines = if $reported {
        [
            "#!/bin/sh"
            'mkdir -p "$CARGO_TARGET_DIR/release"'
            ('printf %s "fresh artifact" > "$CARGO_TARGET_DIR/release/' + $name + '"')
            ('echo "{\"reason\":\"compiler-artifact\",\"filenames\":[\"$CARGO_TARGET_DIR/release/' + $name + '\"]}"')
        ]
    } else {
        [
            "#!/bin/sh"
            "exit 0"
        ]
    }
    ($lines | str join (char nl)) + (char nl) | save -f ($bin | path join "cargo")
    ^chmod +x ($bin | path join "cargo")
}

# Runs the packaging script in the staged workspace with a pinned environment.
def run-package [stage: path, target: path, bin: path] {
    with-env { PATH: ($env.PATH | prepend $bin), CARGO_TARGET_DIR: $target } {
        do {
            cd $stage
            ^($nu.current-exe) scripts/package-plugin.nu
        } | complete
    }
}

# Returns the packaged library's contents.
def archive-library [archive: path, name: string] {
    let member = (
        ^unzip -Z1 $archive
        | lines
        | where {|line| ($line | path basename) == $name }
        | get 0?
    )
    if ($member | is-empty) {
        print -e $"the archive does not contain ($name)"
        exit 1
    }
    ^unzip -p $archive $member
}

# Runs the packaging regression: a redirected `CARGO_TARGET_DIR` must not ship
# a stale library, and a build that never reports the library must fail.
def main [] {
    let repository_root = ($env.FILE_PWD | path dirname)
    require-unzip
    let name = (library-name)
    let stage = (^mktemp -d | str trim)
    try {
        stage-workspace $repository_root $stage
        let target = ($stage | path join "alternate-target")
        mkdir $target
        let stale = ($stage | path join "target" "release")
        mkdir $stale
        "stale artifact" | save -f ($stale | path join $name)

        let reporting_bin = ($stage | path join "fake-bin")
        write-fake-cargo $reporting_bin $name true
        let redirected = (run-package $stage $target $reporting_bin)
        if $redirected.exit_code != 0 {
            print -e $"packaging with a redirected target failed:($redirected.stdout)($redirected.stderr)"
            exit 1
        }
        let archive = (glob ($stage | path join "dist" "*.zip") | get 0?)
        if ($archive | is-empty) {
            print -e "packaging produced no archive"
            exit 1
        }
        let packaged = (archive-library $archive $name)
        if $packaged != "fresh artifact" {
            print -e $"the archive does not contain the artifact Cargo reported: ($packaged)"
            exit 1
        }

        let silent_bin = ($stage | path join "silent-bin")
        write-fake-cargo $silent_bin $name false
        let unreported = (run-package $stage $target $silent_bin)
        if $unreported.exit_code == 0 {
            print -e "packaging must fail when Cargo does not report the library"
            exit 1
        }
        if not ($unreported.stderr | str contains "Cargo did not report") {
            print -e $"unexpected failure:($unreported.stdout)($unreported.stderr)"
            exit 1
        }

        print "package-plugin.nu: passed"
    } finally {
        rm -rf $stage
    }
}
