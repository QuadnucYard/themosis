# Themosis

Themosis is a backend-agnostic design-system compiler. It reads design tokens from a strict subset of the [Design Tokens Community Group](https://www.designtokens.org/) JSON format, combines them with component styles written in KDL, and produces canonical theme data for targeted backends.

The project includes a reusable Rust compilation pipeline, a validation and generation CLI, and a Godot backend with an editor plugin and runnable dashboard example. Godot is one targeted backend rather than part of the compiler's core contract.

## Requirements

- Rust 1.95 or newer
- Godot 4.5 or newer to run the Godot integration and example
- [`just`](https://github.com/casey/just) for the convenience commands (optional)

## Quick start

Check the workspace and validate the example theme:

```sh
cargo check --workspace --all-targets
cargo run -p themosis-cli -- check examples/godot/theme/dashboard.kdl
```

`themosis check` validates source loading, parsing, token resolution, and style
semantics.

The CLI enables its `godot` Cargo feature by default. It provides the Godot
build/check target and its runtime options. Building with `--no-default-features`
keeps the backend-agnostic command surface while excluding the Godot target and
its dependencies.

Build the GDExtension and open the example project:

```sh
cargo build -p themosis-godot-plugin
godot --editor --path examples/godot
```

The example's `.gdextension` configuration loads the debug library from the workspace's `target` directory. Rebuild `themosis-godot-plugin` after changing plugin Rust code.

## Theme sources

A root KDL file declares its token documents, imports, and styles:

```kdl
theme Example {
    tokens "example.tokens.json"
    import "buttons.kdl"

    style PrimaryButton target=Button {
        token normal color.primary
        token font_color color.on-primary

        state hover {
            token hover color.primary-hover
        }
    }
}
```

Token documents use typed DTCG-style values. The supported token types are `boolean`, `number`, `string`, `dimension`, and sRGB `color`; aliases use the `{group.token}` form. See [the token format](crates/themosis-tokens/FORMAT.md) and [the KDL format](crates/themosis-kdl/FORMAT.md) for the complete contracts.

## Workspace

| Path                           | Purpose                                              |
| ------------------------------ | ---------------------------------------------------- |
| `crates/themosis-core`         | Format-independent domain types                      |
| `crates/themosis-tokens`       | DTCG-style JSON token parser                         |
| `crates/themosis-kdl`          | KDL component-style parser                           |
| `crates/themosis-compiler`     | Token resolution and semantic compilation            |
| `crates/themosis`              | Source loading and end-to-end compilation facade     |
| `crates/themosis-cli`          | Source validation and targeted artifact builds       |

## Development

The common commands are available through `just`:

```sh
just check           # check all workspace targets
just test            # run all workspace tests
just ci              # run the local CI-equivalent checks
```
