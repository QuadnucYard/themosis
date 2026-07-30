# AGENTS.md

## Project overview

Themosis is a backend-agnostic Rust design-system compiler for DTCG-style JSON tokens and KDL component styles. It produces canonical theme data for targeted backends. Godot 4.5 is one supported target; keep format and semantic behavior in the format/core/compiler crates and target-specific conversion in backend crates.

## Repository map

- `crates/themosis-core`: format-independent domain values and compiled theme types.
- `crates/themosis-tokens`: strict JSON token parsing. Its public contract is in `FORMAT.md`.
- `crates/themosis-kdl`: KDL style parsing. Its public contract is in `FORMAT.md`.
- `crates/themosis-compiler`: pure token resolution, style inheritance, and diagnostics.
- `crates/themosis`: source providers, import discovery, path safety, and the end-to-end facade.
- `crates/themosis-cli`: source checking and targeted artifact generation.
- `crates/themosis-godot`: reusable Godot `.tres` generation and validation. Supported mappings are in `MAPPINGS.md`.
- `crates/themosis-godot-plugin`: live Godot conversion and the GDExtension API.

## Commands

Run commands from the repository root.

```sh
just fmt
just check
just test
just ci
```

The equivalent commands, without `just`, are:

```sh
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace
```

Useful focused commands include:

```sh
cargo test -p <crate-name>
cargo run -p themosis-cli -- check examples/godot/theme/dashboard.kdl
```

## Implementation guidelines

- Preserve crate boundaries. `themosis-core` performs no parsing or filesystem access, and `themosis-compiler` remains a pure semantic layer.
- Keep source loading root-relative and reject paths that escape the theme root.
- Prefer deterministic collections and diagnostics; the existing pipeline uses `BTreeMap` and `BTreeSet` intentionally.
- Return structured, actionable errors instead of silently ignoring invalid input or unsupported Godot mappings.
- Follow the workspace lint configuration in the root `Cargo.toml`. Public Rust APIs should have documentation. Should ensure comments for unobvious implementation details.
- Keep dependencies centralized in `[workspace.dependencies]` when they are shared, and pin Godot-facing behavior to the supported Godot/API version.
- Update the relevant format or mapping document whenever a public source contract or Godot conversion changes.

## Testing expectations

- Add unit tests near pure parsing or compilation logic.
- Add fixture-driven tests for accepted and rejected source syntax.
- Add facade tests for imports, source discovery, path handling, and end-to-end compilation.
- Add backend or headless Godot tests for native mapping changes.
- Run the narrowest relevant test while iterating, then run `just ci` before handing off a completed change.
- If Rust formatting or lint-sensitive code changed, also run `cargo fmt --all --check` and the relevant Clippy command.

## Change checklist

Before finishing:

1. Confirm the change lives in the correct crate and does not introduce an engine dependency into the compiler layers.
2. Add or update tests that exercise both success and failure behavior.
3. Update `FORMAT.md`, `MAPPINGS.md`, the example, or `README.md` if user-visible behavior changed.
4. Run the relevant focused tests and `just ci`.
5. Report any Godot or platform-specific checks that could not be run.
