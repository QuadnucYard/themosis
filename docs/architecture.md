# Architecture

Themosis is a compiler with pluggable frontends and one backend crate per
target. Engine concerns stay out of the format and compiler crates; that
boundary is what lets one compiled theme feed the Godot addon, the CLI, and
future backends.

## Crates

| Crate | Responsibility |
| --- | --- |
| `themosis-core` | Format-independent domain values and compiled theme types |
| `themosis-tokens` | Strict DTCG-style JSON parsing |
| `themosis-kdl` | KDL 2 style parsing |
| `themosis-compiler` | Pure token resolution, style inheritance, diagnostics |
| `themosis` | Source providers, import discovery, path safety, end-to-end facade |
| `themosis-godot` | Portable Godot build plans, validation, runner protocol |
| `themosis-godot-plugin` | GDExtension: native builder, editor integration, runners |
| `themosis-cli` | Engine-independent checks and project-aware Godot commands |

Only `themosis-godot-plugin` links godot-rust. `themosis-core` performs no
parsing and no filesystem access, `themosis-compiler` is a pure semantic layer,
and `themosis-godot` produces portable plans that never touch a running engine.

## Plugin layering

`themosis-godot-plugin` is layered internally as `native` (engine-side
services), `project` (project state), `editor` (engine classes), and `runners`
(Godot main loops). The dependency rule is one-way: `editor` and `runners` use
the service layers, and no service depends on editor classes or on a main loop.
The imported-artifact contract — metadata keys, fingerprint algorithm, and the
save/load helpers — has a single owner in `native`, so the importer that writes
artifacts and the consumers that validate them cannot disagree about
serialization.

## Features

| Feature | Default | Purpose |
| --- | --- | --- |
| `test-support` | off | `ThemosisBackendTests` probes that the Godot-backed suites call from one-line GDScript entry points |

Distributed builds enable no features: `just package-plugin` builds with the
default feature set, so packaged libraries contain only production classes.

## Tests

Engine-independent suites run under `just test`. `just test-godot` builds the
plugin with the probes, pins that build's library through
`THEMOSIS_GODOT_LIBRARY`, and runs the runtime-backed CLI and plugin suites
against throwaway projects, with the gallery import gate last. A missing
engine, library, or probe build skips locally with a message and fails when
`THEMOSIS_REQUIRE_GODOT` is set, so the dedicated Godot job cannot pass by
skipping.

## Packaging

`crates/themosis-godot-plugin/addon/` holds the distribution assets: the addon
guide and the `.gdextension` template. `scripts/package-plugin.nu` stages them
together with the license, the format and mapping documents, and the built
libraries into `addons/themosis/`, verifies the archive layout, and writes
`dist/themosis-godot-<version>[-<platform>-<arch>].zip`. The example project
consumes the same layout through `examples/godot/addons/themosis/`.

## Scripts

The repository scripts are [Nushell](https://www.nushell.sh/) programs; run
them with `nu`:

| Script | Purpose |
| --- | --- |
| `scripts/godot-tests.nu` | builds the plugin with the probes, pins the artifact and engine, runs the runtime-backed suites, and validates the gallery import gate |
| `scripts/test-godot-tests.nu` | verifies extension registration before import, runner completion, diagnostics, failure propagation, and staged-project cleanup |
| `scripts/package-plugin.nu` | packages the host addon, or a cross-platform bundle from `--native-root` |
| `scripts/test-plugin-package.nu` | proves packaging ships the artifact Cargo reports |
| `scripts/verify-plugin-package.nu` | verifies a packaged archive's layout (`local` or `bundle`) |
