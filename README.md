# Themosis

Themosis is a backend-agnostic design-system compiler. It resolves strict
DTCG-style JSON tokens and KDL 2 component styles into canonical theme data,
then maps that data through a selected backend. Godot 4.5+ is one supported
target; engine concerns stay outside the format and compiler crates.

## Requirements

- Rust 1.95 or newer
- Godot 4.5 or newer for the Godot integration
- [`just`](https://github.com/casey/just) for convenience commands (optional)
- `zip` and `unzip` for addon packaging

## Godot quick start

Build the GDExtension and open the example:

```sh
cargo build -p themosis-godot-plugin
godot --editor --path examples/godot
```

The example has two importable roots:

```text
theme/light.tms ─┐
                 ├─ shared KDL component modules
theme/dark.tms ──┘  + common and palette-specific JSON tokens
```

The editor addon imports each `.tms` directly as a native Godot `Theme`. Run
the example and use its Light and Dark buttons to switch between
`res://theme/light.tms` and `res://theme/dark.tms`; no startup compilation or
generated `.tres` path is needed.

The development `.gdextension` loads the debug library from the workspace
`target` directory. Rebuild `themosis-godot-plugin` after changing Rust code.

## Use in another Godot project

Extract `themosis-godot-<version>.zip` into the project root and enable
**Themosis** under **Project Settings → Plugins**. The plugin entry must be at
`res://addons/themosis/plugin.cfg`.

Create one `.tms` root for each concrete theme. Paths containing `/` remain
quoted; ordinary KDL 2 values use bare strings:

```kdl
// res://theme/light.tms
theme Application {
    tokens "tokens/common.tokens.json"
    tokens "tokens/light.tokens.json"
    import "styles/buttons.kdl"
}
```

Shared imports are fragments and do not repeat the theme wrapper:

```kdl
// res://theme/styles/buttons.kdl
style Button target=Button {
    token normal surface.raised
    number font_size 16
}

style PrimaryButton target=Button extends=Button {
    token normal brand.primary
}
```

`style Button target=Button` supplies defaults for every `Button`. A different
style name creates a Godot type variation, selected with
`theme_type_variation = &"PrimaryButton"`.

Reference imported roots exactly like ordinary resources:

```gdscript
const THEMES := {
    &"light": preload("res://theme/light.tms"),
    &"dark": preload("res://theme/dark.tms"),
}

func use_theme(name: StringName) -> void:
    theme = THEMES[name]
```

Godot owns importer output under `.godot/imported`. The addon calls
`ThemosisThemeGenerator` in the GDExtension directly—it never invokes the Rust
CLI. Its dock provides **Reimport**, **Reimport all**, native previews, and
clickable structured diagnostics. Persisted dependency fingerprints rebuild
every affected root after shared KDL or JSON changes, including across editor
restarts.

When a visible stable output is useful, choose **Materialize…** and a confined
`res://….tres` path such as `res://theme/generated/light.tres`. **All…** writes
one file per root into a selected directory. Profile configuration in
`res://themosis.godot.json` is retained for deterministic headless
materialization:

```sh
godot --headless --editor --path . --import
godot --headless --path . \
  --script res://addons/themosis/build.gd -- --all
```

Both editor materialization and the headless builder call the extension. Output
and source paths reject empty, `.`, `..`, and backslash segments.

Runtime `generate()` remains available for mod or user-authored themes, but
normal application themes should use imported `.tms` resources so compilation
does not occur at startup.

## Standalone CLI

The CLI is independent of the addon and remains useful for backend-neutral
checks or workflows that intentionally do not load the GDExtension:

```sh
cargo run -p themosis-cli -- check examples/godot/theme/light.tms
cargo run -p themosis-cli -- build --target godot \
  --project examples/godot \
  --output res://.themosis/light.tres \
  examples/godot/theme/light.tms
```

`check` validates loading, KDL/JSON parsing, token resolution, and style
semantics. `--target godot` additionally validates against a running Godot
engine. The standalone Godot builder uses `--godot FILE`, then
`THEMOSIS_GODOT_BINARY`, then `godot`/`godot4`; it supports an exact
`--require-godot-version` and a configurable `--godot-timeout`.

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
| `crates/themosis-cli` | Validation and standalone artifact builds |
| `crates/themosis-godot` | Portable Godot plans and native builder |
| `crates/themosis-godot-plugin` | GDExtension and live Godot objects |
| `examples/godot` | Multi-theme switching demo and addon sources |

## Development

```sh
just check
just test
just ci
just package-plugin
```

`just package-plugin` creates a host development archive under `dist/`. Tagged
releases assemble Linux x86_64, Windows x86_64, and macOS x86_64/arm64 native
libraries into one archive. See the [example guide](examples/godot/README.md)
for the demo and packaging workflow.
