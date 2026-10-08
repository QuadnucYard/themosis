# Themosis Godot component-gallery example

This project demonstrates the recommended importer-first integration: two
`.tms` roots become native Godot `Theme` assets and a component gallery
switches between them.

From the repository root:

```sh
cargo build -p themosis-godot-plugin
godot --editor --path examples/godot
```

Run the project and press **Light** or **Dark**:

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

The bundled Themosis plugin auto-registers while the editor loads the extension
and contributes its dock; the
[addon guide](../../crates/themosis-godot-plugin/addon/README.md) covers its
reimport, preview, diagnostic, and materialization features. Dependency
fingerprints survive editor restarts: a change to shared `buttons.kdl` refreshes
both roots, while a change to `light.tokens.json` refreshes only `light.tms`.
The optional `themosis.godot.json` profiles configure stable headless
materialization.

## Package the addon

```sh
just package-plugin
```

This writes a development archive under `dist/`. The example consumes the addon
the way a project does: the manifest under `addons/themosis/` registers the
extension, and the archive carries the same layout. See the
[addon guide](../../crates/themosis-godot-plugin/addon/README.md) for
installation and export details.

## Validate imports before export

Before exporting imported `.tms` assets, run the addon's validation gate; it
exits nonzero for invalid or stale imports, so stop the export pipeline when it
fails. The [addon guide](../../crates/themosis-godot-plugin/addon/README.md)
documents the commands.
