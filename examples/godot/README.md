# Themosis Godot component-gallery example

This project demonstrates the recommended importer-first integration: two
`.tms` roots become native Godot `Theme` assets and a component gallery
switches between them.

From the repository root:

```sh
cargo build -p themosis-godot-plugin
godot --editor --path examples/godot
```

Run the project and press **Light** or **Dark**. The application still switches
the entire native theme with only this:

```gdscript
const THEMES := {
    &"light": preload("res://theme/light.tms"),
    &"dark": preload("res://theme/dark.tms"),
}

func set_application_theme(name: StringName) -> void:
    theme = THEMES[name]
```

The source graph is intentionally small:

```text
theme/light.tms + tokens/light.tokens.json ─┐
theme/dark.tms  + tokens/dark.tokens.json  ─┤
tokens/common.tokens.json                  ─┼─ native Theme assets
styles/{surfaces,typography,buttons}.kdl   ─┤
styles/{layout,inputs,feedback}.kdl        ─┤
assets/{ui_font,focus_ring,chevron_down}   ─┘
```

Each concrete theme gets one root. Shared `.kdl` files are wrapper-free KDL 2
fragments, so they can be reused by both roots. Styles named for their target
set native defaults; the few opt-in styles are Godot type variations.

The gallery covers `Panel`, `PanelContainer`, `Label`, `Button`, `LineEdit`,
`OptionButton`, `CheckBox`, `ProgressBar`, `MarginContainer`, `GridContainer`,
and both box-container directions. Together they demonstrate every native item
category currently supported by the backend:

| Source value | Native item | Gallery example |
| --- | --- | --- |
| color token | color | text, caret, and selection colors |
| color token | stylebox | panels, fields, buttons, and progress fill |
| whole `px` dimension token | constant | margins and container separation |
| whole number token | font size | labels and interactive controls |
| KDL resource | font | shared `SystemFont` resource |
| KDL resource | icon | `OptionButton` chevron SVG |
| KDL resource | stylebox | shared focus ring |

The **Not mapped yet** area deliberately keeps placeholders visible for
boolean/string values and fractional or `rem` dimensions. Putting those values
in the KDL would correctly fail Godot validation, so the example calls out the
missing support instead of silently omitting it.

## Editor workflow

Enable the bundled **Themosis** plugin and open its dock. It discovers every
`.tms` recursively and offers:

- **Reimport** for the selected root and **Reimport all** globally.
- A live native preview and structured diagnostics.
- **Materialize…** for an explicit stable `.tres` output.
- **All…** to materialize every root into one directory.

Imported output belongs to Godot under `.godot/imported`; scenes reference the
source paths directly. Dependency fingerprints survive editor restarts. A
change to shared `buttons.kdl` refreshes both roots, while a change to
`light.tokens.json` refreshes only `light.tms`.

The editor scripts call `ThemosisThemeGenerator` in the GDExtension directly
and never spawn the CLI. The optional `themosis.godot.json` profiles are only
stable headless materialization presets:

```sh
godot --headless --editor --path examples/godot --import
godot --headless --path examples/godot \
  --script res://addons/themosis/build.gd -- --all
```

## Package the addon

```sh
just package-plugin
```

This creates a platform-qualified development archive under `dist/`. Tagged
releases combine Linux x86_64, Windows x86_64, and macOS x86_64/arm64 native
libraries. Extract an archive into another project and enable **Themosis**
under **Project Settings → Plugins**.

See the [addon guide](addons/themosis/README.md) for installation, export,
materialization, and optional runtime compilation details.
