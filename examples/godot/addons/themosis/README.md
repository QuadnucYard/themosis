# Themosis Godot addon

The addon imports Themosis sources as native Godot `Theme` resources. It
requires Godot 4.5 or newer and targets the 4.5 GDExtension API baseline.

## Install

Extract the release archive into the project root so the plugin entry is
`res://addons/themosis/plugin.cfg`, then enable **Themosis** under **Project
Settings → Plugins**.

Combined release archives include native libraries for Linux x86_64, Windows
x86_64, and macOS x86_64/arm64. Host development archives contain only the
platform named in their filename.

## Recommended: import `.tms` roots

Use one `.tms` root for each concrete theme:

```kdl
// res://theme/light.tms
theme Application {
    tokens "tokens/common.tokens.json"
    tokens "tokens/light.tokens.json"
    import "styles/controls.kdl"
}
```

Shared KDL 2 imports are fragments and need no repeated theme declaration:

```kdl
// res://theme/styles/controls.kdl
style Button target=Button {
    token normal surface.raised
    token font_color text.primary
    number font_size 16
}

style PrimaryButton target=Button extends=Button {
    token normal brand.primary
    token font_color text.on-accent
}
```

Quotes are omitted for valid KDL 2 bare strings and retained for paths
containing `/`. The complete source contracts are bundled under `docs/`.

The importer compiles each root through `ThemosisThemeGenerator` in the loaded
GDExtension and saves the result in Godot's managed `.godot/imported` cache. It
does not execute the Rust CLI. Reference the source path as a normal resource:

```gdscript
const LIGHT := preload("res://theme/light.tms")
const DARK := preload("res://theme/dark.tms")

func select_theme(dark: bool) -> void:
    theme = DARK if dark else LIGHT
```

A style named exactly like its target sets defaults for that native control
type. Other names become opt-in `theme_type_variation` values. This permits a
theme to style ordinary `Button` nodes without adding a variation to every
scene node.

## Themosis dock

The dock discovers `.tms` roots recursively and provides:

- **Reimport** and **Reimport all** for on-demand regeneration.
- Per-root importing, stale, up-to-date, and failed status.
- A native Theme preview.
- Structured diagnostics with source paths and stable codes.
- **Materialize…** and **All…** for explicit `.tres` output.

The extension reports the exact root, imported KDL, token JSON, and `res://`
resource dependency graph. Each imported Theme persists a fingerprint of that
graph. The plugin compares it after editor scans and watches it while the
editor remains open, so dependency invalidation works across restarts and only
affected roots reimport. Invalid source attempts remain visible in the dock.

## Choosing an output path

Normal scenes should reference `res://theme/light.tms`; Godot chooses the cache
path and includes the imported native Theme during export. There is no output
setting for this primary workflow.

Use **Materialize…** when another tool or deployment policy needs a visible,
stable resource such as `res://theme/generated/light.tres`. Paths must remain
inside `res://`, end in `.tres`, and contain no empty, `.`, `..`, or backslash
segments. Materialization writes a temporary sibling and replaces the old file
only after compilation and serialization succeed.

## Headless materialization profiles

`res://themosis.godot.json` is an optional set of deterministic headless
presets. It is not needed by the importer or dock:

```json
{
  "active_profile": "light",
  "profiles": [
    {
      "auto_refresh": false,
      "build_on_start": false,
      "enabled": true,
      "name": "light",
      "output": "res://theme/generated/light.tres",
      "preview": "none",
      "source": "res://theme/light.tms"
    }
  ],
  "version": 1
}
```

Run one or all profiles before an export that consumes materialized files:

```sh
godot --headless --editor --path . --import
godot --headless --path . \
  --script res://addons/themosis/build.gd -- --profile light
# Or replace the final arguments with: -- --all
godot --headless --editor --path . --import
godot --headless --path . --export-release "Desktop" build/game
```

The headless script also calls the GDExtension directly. `--all` continues
through every enabled profile, reports every failure, and exits nonzero if any
profile fails. `auto_refresh`, `build_on_start`, and `preview` remain in the
version-1 profile schema for compatibility with the former profile-driven
editor workflow; importer-first editor behavior does not use them.

## Export behavior

Imported `.tms` resources referenced by scenes or scripts are exported as
native Godot resources. Raw KDL and JSON sources are not needed at runtime.

If the game intentionally calls `generate()` at runtime for mod or
user-authored themes, include the relevant `*.tms`, `*.kdl`, and token JSON
files in the export preset's non-resource filters and ship the matching native
addon library for each platform.

## Optional runtime API

```gdscript
var generator := ThemosisThemeGenerator.new()
var result: Dictionary = generator.generate_result("res://theme/user.tms")
if result["ok"]:
    theme = result["theme"] as Theme
else:
    for diagnostic in result["diagnostics"]:
        push_error("[%s] %s" % [diagnostic["code"], diagnostic["message"]])
```

`generate()` remains as a compact compatibility API and exposes its last error,
dependencies, and diagnostics through getters. Runtime compilation is useful
for mutable themes, but imported resources are the simpler application default.

## Optional standalone CLI

The Rust CLI is a separate integration choice for workflows that deliberately
do not load the addon. It starts a selected headless Godot executable and uses
the same portable plan and native builder. It is not called by any addon editor
or headless script.
