# Themosis

Themosis is a backend-agnostic design-system compiler. It resolves strict
DTCG-style JSON tokens and KDL 2 component styles into canonical theme data,
then maps that data through a selected backend. Godot 4.5+ is one supported
target; engine concerns stay outside the format and compiler crates.

## Requirements

- Rust 1.95 or newer
- Godot 4.5 or newer for the Godot integration
- [Nushell](https://www.nushell.sh/) for the repository scripts
- [`just`](https://github.com/casey/just) for convenience commands (optional)
- `zip` and `unzip` for addon packaging

## Godot quick start

Build the GDExtension and open the example:

```sh
cargo build -p themosis-godot-plugin
godot --editor --path examples/godot
```

The example imports `theme/light.tms` and `theme/dark.tms` as native Godot
`Theme` resources; its Light and Dark buttons switch between them, with no
startup compilation or generated `.tres` path. The development `.gdextension`
loads the debug library from the workspace `target` directory, so rebuild
`themosis-godot-plugin` after changing Rust code.

## Use in another Godot project

Extract `themosis-godot-<version>.zip` into the project root. The native editor
plugin activates with the GDExtension library, so there is nothing to enable
under **Project Settings → Plugins**. The
[addon guide](crates/themosis-godot-plugin/addon/README.md) covers the dock,
export, and materialization.

Create one `.tms` root for each concrete theme. Paths containing `/` stay
quoted; ordinary KDL 2 values use bare strings:

```kdl
// res://theme/light.tms
theme Application {
    tokens "tokens/common.tokens.json"
    tokens "tokens/light.tokens.json"
    import "styles/buttons.kdl"
}
```

Shared imports are wrapper-free fragments. A style named exactly like its
target (`Button`) supplies defaults for that native control type; any other
name becomes an opt-in `theme_type_variation`. Reference the root like any
other resource:

```gdscript
const LIGHT := preload("res://theme/light.tms")
```

The importer compiles each root inside the GDExtension and stores the result in
Godot's managed cache; compilation happens in the editor, not at startup, and
there is no scripting API. Persisted dependency fingerprints rebuild affected
roots after shared KDL or JSON changes, including across editor restarts.
**Materialize…** and the optional `res://themosis.godot.json` profiles cover
explicit and headless output.

## Standalone CLI

`themosis check` validates loading, KDL/JSON parsing, token resolution, and
style semantics without a running engine:

```sh
cargo run -p themosis-cli -- check examples/godot/theme/light.tms
```

The `godot` subcommands drive the same native builder as the editor importer
and run inside a project that ships the addon and has been imported at least
once:

```sh
cargo run -p themosis-cli -- godot check \
  --project examples/godot \
  examples/godot/theme/light.tms

cargo run -p themosis-cli -- godot build \
  --project examples/godot \
  --output res://.themosis/light.tres \
  examples/godot/theme/light.tms
```

They launch `godot --headless --path PROJECT --main-loop ThemosisCliRunner` and
resolve the engine through `--godot FILE`, then `THEMOSIS_GODOT_BINARY`, then
`godot`/`godot4`; `--require-version` and `--timeout` are supported. The
default-on `godot` Cargo feature gates these commands.

## Source contracts

Token documents support `boolean`, `number`, `string`, `dimension`, and sRGB
`color` values; aliases use `{group.token}`. See the
[token contract](crates/themosis-tokens/FORMAT.md),
[KDL contract](crates/themosis-kdl/FORMAT.md), and
[Godot mappings](crates/themosis-godot/MAPPINGS.md).

## Workspace

| Path | Purpose |
| --- | --- |
| `crates/themosis-core` | Format-independent domain types |
| `crates/themosis-tokens` | Strict DTCG-style JSON parser |
| `crates/themosis-kdl` | KDL 2 style parser |
| `crates/themosis-compiler` | Pure token and style compilation |
| `crates/themosis` | Safe source loading and end-to-end facade |
| `crates/themosis-cli` | Validation and project-aware Godot builds |
| `crates/themosis-godot` | Portable Godot plans and the runner protocol |
| `crates/themosis-godot-plugin` | GDExtension: native builder, editor integration, and runners |
| `examples/godot` | Multi-theme demo that consumes the addon layout |

The [architecture guide](docs/architecture.md) covers crate boundaries, the
plugin's internal layers, features, and packaging.

## Development

```sh
just fmt-check    # verify formatting
just clippy       # lint every target, denying warnings like CI
just test         # engine-independent suites
just test-godot   # Godot-backed plugin and CLI suites with the test probes
just ci           # the checks continuous integration runs
just package-plugin
```

`just test-godot` builds `themosis-godot-plugin` with the `test-support`
feature and pins the resulting library through `THEMOSIS_GODOT_LIBRARY`,
failing instead of skipping when Godot or the library is missing; set
`THEMOSIS_GODOT_BINARY` to select a specific engine build. `just package-plugin`
creates a host development archive under `dist/`, while tagged releases
assemble the cross-platform archive. The
[example guide](examples/godot/README.md) documents the demo and packaging
workflow.

### Validate imported themes before export

After importing the project, run the addon's validation gate before exporting:

```sh
godot --headless --editor --path examples/godot --import
godot --headless --path examples/godot --main-loop ThemosisCheckRunner
```

The check exits nonzero for invalid sources and stale imported themes, so build
pipelines should stop on failure.
