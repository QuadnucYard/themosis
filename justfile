alias f := fmt
alias c := check
alias t := test

# List available project commands.
default:
    @just --list=

# Format all workspace code.
fmt:
    cargo fmt --all

# Verify formatting without rewriting files.
fmt-check:
    cargo fmt --all --check

# Check every target in the workspace.
check:
    cargo clippy --workspace --all-targets

# Check every target, failing on any warning, exactly like CI.
clippy:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy --workspace --all-targets --features themosis-godot-plugin/test-support -- -D warnings

# Run all workspace tests.
test:
    cargo test --workspace

# Run the Godot-backed CLI and plugin suites against a fresh test-support build.
test-godot:
    nu scripts/godot-tests.nu

# Run the same checks expected in continuous integration.
ci: fmt-check clippy test test-plugin-package test-godot-script

# Verify the import gate waits for Godot and reports runner failures.
test-godot-script:
    nu scripts/test-godot-tests.nu

# Verify host packaging ships the artifact Cargo reports.
test-plugin-package:
    nu scripts/test-plugin-package.nu

# Build and package the Godot addon for the current host only.
package-plugin:
    nu scripts/package-plugin.nu

# Assemble a cross-platform addon from a native/<platform>/<arch> directory.
package-plugin-bundle native_root:
    nu scripts/package-plugin.nu --native-root "{{ native_root }}"
