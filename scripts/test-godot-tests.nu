#!/usr/bin/env nu
# Checks extension registration, runner completion, and failure propagation.

def main [] {
    let repository_root = ($env.FILE_PWD | path dirname)
    let stage = (^mktemp -d | str trim)
    try {
        let godot = ($stage | path join "godot")
        [
            "#!/bin/sh"
            'while [ "$#" -gt 0 ]; do'
            '  case "$1" in'
            '    --path) shift; project="$1" ;;'
            '    --import) cat "$project/.godot/extension_list.cfg" >> "$GATE_IMPORT_RECORD"; exit 1 ;;'
            '  esac'
            '  shift'
            'done'
            'printf %s "$project" > "$GATE_PROJECT_RECORD"'
            'sleep 0.2'
            '[ -f "$project/themosis.gdextension" ] || exit 99'
            'echo "runner finished"'
            'echo "runner diagnostic" >&2'
            'exit "$GATE_EXIT_CODE"'
        ] | str join (char nl) | save -f $godot
        ^chmod +x $godot
        let record = ($stage | path join "project-path")
        let imports = ($stage | path join "import-registration")
        for status in [0 42] {
            "" | save -f $imports
            let result = (
                with-env {
                    GATE_PROJECT_RECORD: $record,
                    GATE_IMPORT_RECORD: $imports,
                    GATE_EXIT_CODE: ($status | into string),
                    GATE_BINARY: $godot,
                    GATE_REPOSITORY: $repository_root
                } {
                    do {
                        ^($nu.current-exe) --no-config-file -c 'source scripts/godot-tests.nu; run-import-gate $env.GATE_BINARY "fake library" $env.GATE_REPOSITORY'
                    } | complete
                }
            )
            if (open --raw $imports | lines) != ["res://themosis.gdextension" "res://themosis.gdextension"] {
                error make {msg: "the extension was not registered before both import passes"}
            }
            if not ($result.stdout | str contains "runner finished") {
                error make {msg: $"runner did not finish before cleanup: ($result)"}
            }
            if not ($result.stderr | str contains "runner diagnostic") {
                error make {msg: "runner diagnostics were lost"}
            }
            if $result.exit_code != $status {
                error make {msg: $"unexpected gate status: ($result)"}
            }
            if (open --raw $record | path exists) {
                error make {msg: "gate did not clean up its staged project"}
            }
        }
        print "godot-tests.nu: passed"
    } finally {
        rm -rf $stage
    }
}
