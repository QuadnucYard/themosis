# AGENTS.md

## Project overview

Themosis is a backend-agnostic Rust design-system compiler for DTCG-style JSON tokens and KDL component styles. It produces canonical theme data for targeted backends. Godot 4.5 is one supported target; keep format and semantic behavior in the format/core/compiler crates and target-specific conversion in backend crates.

## Repository map

- `crates/themosis-core`: format-independent domain values and compiled theme types.
- `crates/themosis-tokens`: strict JSON token parsing. Its public contract is in `FORMAT.md`.
- `crates/themosis-kdl`: KDL style parsing. Its public contract is in `FORMAT.md`.
- `crates/themosis-compiler`: pure token resolution, style inheritance, and diagnostics.
- `crates/themosis`: source providers, import discovery, path safety, and the end-to-end facade.
- `crates/themosis-cli`: source checking and project-aware Godot commands.
- `crates/themosis-godot`: portable Godot build plans, validation, and the versioned runner protocol. Supported mappings are in `MAPPINGS.md`.
- `crates/themosis-godot-plugin`: live Godot conversion and the GDExtension API. Internally layered as `native` (engine services), `project` (project state), `editor` (engine classes), and `runners` (Godot main loops); see `docs/architecture.md`.
- `examples/godot`: executable demo that consumes the addon layout and the packaging workflow.
- `scripts/package-plugin.nu`: platform-specific addon packaging; `scripts/godot-tests.nu` runs the Godot-backed suites; `scripts/test-plugin-package.nu` proves packaging ships the artifact Cargo reports. Repository scripts are Nushell programs, verified by `scripts/verify-plugin-package.nu`.
- `docs/architecture.md`: crate boundaries, plugin layering, features, test suites, and packaging.

## Commands

Run commands from the repository root.

```sh
just fmt-check
just clippy
just test
just ci
```

The equivalent commands, without `just`, are:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Useful focused commands include:

```sh
cargo test -p <crate-name>
cargo run -p themosis-cli -- check examples/godot/theme/light.tms
cargo run -p themosis-cli -- godot check --project examples/godot examples/godot/theme/light.tms
cargo run -p themosis-cli -- godot build --project examples/godot --output res://.themosis/light.tres examples/godot/theme/light.tms
cargo build -p themosis-godot-plugin
just test-godot
just package-plugin
```

The `check` command is engine-independent. The `godot` commands load a project
and need the Themosis addon installed and imported at least once, so they
require a usable Godot environment. Packaging additionally requires `zip` and
produces an archive under `dist/`.

`just test-godot` (or `scripts/godot-tests.nu`) builds
`themosis-godot-plugin` with the non-default `test-support` feature, pins that
library through `THEMOSIS_GODOT_LIBRARY`, runs the runtime-backed CLI and plugin
suites, and then runs the gallery import gate. Missing prerequisites fail
instead of skipping, and `THEMOSIS_GODOT_BINARY` selects the engine.

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
- Engine-facing changes: extend `crates/themosis-godot-plugin/tests` or
  `crates/themosis-cli/tests`; keep Godot-backed suites on `tests/support`
  helpers and reserve `tests/gallery.rs` for demo smoke tests.
- Run the narrowest relevant test while iterating, then run `just ci` before handing off a completed change.
- Run `just test-godot` when Rust code changes an engine-facing surface, an
  editor class, or the packaging layout.

## Change checklist

Before finishing:

1. Confirm the change lives in the correct crate and does not introduce an engine dependency into the compiler layers.
2. Add or update tests that exercise both success and failure behavior.
3. Update `FORMAT.md`, `MAPPINGS.md`, the example, or `README.md` if user-visible behavior changed.
4. Run the relevant focused tests and `just ci`; run `just test-godot` for engine-facing or packaging changes.
5. Report any Godot or platform-specific checks that could not be run.
