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
