# Themosis Godot addon

The addon imports Themosis sources as native Godot `Theme` resources. It
requires Godot 4.5 or newer and targets the 4.5 GDExtension API baseline.

## Install

Extract the release archive into the project root. The native editor plugin
registers while the editor loads the GDExtension library, so nothing has to be
enabled under **Project Settings → Plugins**.

Combined release archives include native libraries for Linux x86_64, Windows
x86_64, and macOS x86_64/arm64. Host development archives contain only the
platform named in their filename.

## Native architecture

Everything ships inside the GDExtension library: the editor plugin, the `.tms`
importer, the dock, profile storage, the builder, and the headless
build/validation runners. A release contains no GDScript, so the plugin cannot
be toggled from the plugins dialog and always follows the loaded extension. The
addon exposes no scripting API of its own. GDScript in this repository is
limited to the example's demo control and the probes behind the non-default
`test-support` feature; none of it is packaged.

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

Shared KDL 2 imports are wrapper-free fragments:

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

The complete source contracts, including when KDL quoting is required, are
bundled under `docs/`.

The importer compiles each root inside the loaded GDExtension and saves the
result in Godot's managed `.godot/imported` cache. It does not execute the Rust
CLI. Reference the source path as a normal resource:

```gdscript
const LIGHT := preload("res://theme/light.tms")
const DARK := preload("res://theme/dark.tms")

func select_theme(dark: bool) -> void:
    theme = DARK if dark else LIGHT
```

A style named exactly like its target sets defaults for that native control
type. Other names become opt-in `theme_type_variation` values, so a theme can
style ordinary `Button` nodes without adding a variation to every scene node.

## Themosis dock

The dock discovers `.tms` roots recursively and provides:

- **Reimport** and **Reimport all** for on-demand regeneration.
- Per-root importing, stale, up-to-date, and failed status.
- A native Theme preview.
- Structured diagnostics with source paths and stable codes.
- **Materialize…** and **All…** for explicit `.tres` output.

Each imported Theme persists its dependency graph and a fingerprint of it.
Startup rechecks the saved paths, and the plugin keeps watching while the
editor is open, so a change reimports only the affected roots, including across
restarts. Import freshness is separate from validation and materialization
results: a successful materialization does not make a stale import current.
Discovery skips hidden and linked directories.

## Choosing an output path

Normal scenes should reference `res://theme/light.tms`; Godot chooses the cache
path and includes the imported native Theme during export. There is no output
setting for this primary workflow.

Use **Materialize…** when another tool or deployment policy needs a visible,
stable resource such as `res://theme/generated/light.tres`. Paths must remain
inside `res://`, end in `.tres`, and contain no empty, `.`, `..`, or backslash
segments. Materialization writes a unique temporary sibling and replaces the
old file only after compilation and serialization succeed.

## Validate imports before export

Godot's `--import` exit status alone does not reliably reflect importer
failures. Validate the current sources and their imported fingerprints before
exporting:

```sh
godot --headless --editor --path . --import
godot --headless --path . --main-loop ThemosisCheckRunner
```

The check exits nonzero for invalid sources and stale imports. Keep these
commands in a build pipeline that stops on failure; last valid previews remain
available without being treated as successful production imports.

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

Run one profile or all of them before an export that consumes materialized
files:

```sh
godot --headless --editor --path . --import
godot --headless --path . --main-loop ThemosisBuildRunner -- --all
godot --headless --path . --main-loop ThemosisCheckRunner
```

Replace `-- --all` with `-- --profile light` to build a single profile. `--all`
continues through every enabled profile, reports every failure, and exits
nonzero if any profile fails. `auto_refresh`, `build_on_start`, and `preview`
remain in the version-1 profile schema for compatibility with the former
profile-driven editor workflow; the importer-first workflow does not use them.

## Export behavior

Imported `.tms` resources referenced by scenes or scripts are exported as
native Godot resources; raw KDL and JSON sources are not needed at runtime.
Customization happens in the editor: edit the sources, let the importer
rebuild, and export the imported resources. The addon deliberately offers no
runtime compilation API, so exported builds need neither the raw sources nor a
scripting entry point into the compiler.

## Standalone CLI

The Rust CLI drives the same native builder through the loaded addon instead of
implementing its own mapping. Install the addon and import the project at least
once so Godot registers `ThemosisCliRunner`; a missing or unregistered addon
fails before the engine starts. The bundled Godot mappings guide documents the
commands and their options.
