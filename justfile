alias f := fmt
alias c := check
alias t := test

# List available project commands.
default:
    @just --list=

# Format all workspace code.
fmt:
    cargo fmt --all --check

# Check every target in the workspace.
check:
    cargo check --workspace --all-targets

# Run all workspace tests.
test:
    cargo test --workspace

# Run the same checks expected in continuous integration.
ci: fmt check test

# Build and package the Godot addon for the current host only.
package-plugin:
    ./scripts/package-plugin.sh

# Assemble a cross-platform addon from a native/<platform>/<arch> directory.
package-plugin-bundle native_root:
    ./scripts/package-plugin.sh --native-root "{{native_root}}"
