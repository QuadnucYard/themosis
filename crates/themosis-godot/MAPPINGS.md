# Godot theme mappings

The backend supports Godot versions from the 4.5 baseline onward. Targets must
be `Control` classes registered by the Godot engine performing validation or
generation. A style whose name equals its target writes the native defaults for
that control type; every other style becomes a native type variation whose base
type is its `target`.

```kdl
// Applies to every Button using this Theme.
style Button target=Button {
    token normal surface.raised
}

// Opt-in with Control.theme_type_variation = &"PrimaryButton".
style PrimaryButton target=Button extends=Button {
    token normal brand.primary
}
```

The Godot integration is split across two crates, and one native implementation
serves every runtime path:

- `themosis-godot` validates portable constraints and produces a serializable build plan without depending on `godot-rust`.
- `themosis-godot-plugin` resolves and applies that plan inside Godot. It reads `ThemeDB.get_default_theme()`, walks each target's live `ClassDB` hierarchy, and writes the resolved items onto a native `Theme`. The editor importer, the editor dock and builder, the headless runners, and the native CLI runner (`ThemosisCliRunner`) all construct themes through this one implementation, so they cannot diverge.
- The addon is entirely native: it registers its editor plugin, importer, dock, and runners with the engine and exposes no scripting API. Editor integration, materialization, and headless commands call the same Rust code paths; the only Godot values crossing a boundary are engine notifications (signals) and the native controls and resources the addon owns.
- `themosis-cli` starts a headless Godot process with the target project loaded and `--main-loop ThemosisCliRunner`, passing a versioned JSON request and reading a versioned JSON response. No GDScript participates in mapping or process control.

The runner protocol is defined once in `themosis-godot`'s `runner` module and
carries an explicit schema version independent of the crate versions, so an
incompatible addon is rejected with `protocol_version_mismatch` rather than
misinterpreted.

`themosis-godot` contains neither a version-specific class/property catalog nor a handwritten `.tres` serializer. Godot itself is the metadata and serialization authority. The `godot-rust` dependency belongs only to `themosis-godot-plugin`; the reusable crate remains usable by the CLI without linking Godot.

## Native item names

KDL property names are exact Godot theme-item names for the target control. The portable plan records candidate categories from the compiled value. The native builder intersects those candidates with `ThemeDB.get_default_theme()` across the target's live `ClassDB` hierarchy. A color can therefore resolve to a color item or a stylebox, while a whole pixel value can resolve to a constant or font size without a version-specific catalog.

```kdl
style PrimaryButton target=Button {
    token normal brand.primary
    token font_color text.on-accent
    number font_size 17

    state hover {
        token hover brand.hover
        token font_hover_color text.on-accent
    }
}
```

State blocks group and inherit state-specific items, but the backend does not synthesize item names from the state name. A state must declare the exact native item, such as `hover` or `font_hover_color`. Changing the base item `normal` from inside a state is rejected because Godot has no state-local override for that item.

## Value-driven categories

| Compiled value | Compatible Godot item category | Behavior |
| --- | --- | --- |
| color | color | sets the named color item |
| color | stylebox | changes the background of a duplicated `StyleBoxFlat` default; both CLI and plugin reject missing defaults and other stylebox subclasses instead of silently discarding their behavior |
| whole number or `px` dimension | constant | sets the named constant, including negative constants |
| positive whole number or `px` dimension | font size | sets the named font-size item |
| `res://` or `uid://` resource inheriting `Font` | font | sets the named font item |
| `res://` or `uid://` resource inheriting `Texture2D` | icon | sets the named icon item |
| `res://` or `uid://` resource inheriting `StyleBox` | stylebox | sets the complete named stylebox item |

If a name and value match no native item, mapping fails. If they match more than one category, mapping also fails rather than choosing implicitly. Boolean, string, fractional number, `rem`, missing resources, and unsupported resource types are errors.

The core retains resource references as backend-neutral text. The `res://` and `uid://` requirements above are enforced only by `themosis-godot`.

## Godot editor assets

The addon recognizes `.tms` root files with an `EditorImportPlugin` and saves
each result as a native `Theme` in Godot's `.godot/imported` cache. A scene can
therefore reference `res://theme/light.tms` directly. One `.tms` root per
concrete theme keeps light, dark, and other token compositions independent;
shared component fragments conventionally remain `.kdl` files.

The imported resource stores a deterministic dependency fingerprint. On editor
startup and while the editor is open, the plugin recompiles only roots affected
by changed KDL, token JSON, or referenced `res://` resources. **Reimport** and
**Reimport all** expose the same operation on demand, and a failed reimport
keeps the previous cache so previews stay available. **Materialize** is the
explicit alternative when a stable, visible `.tres` output is required.

Targeted commands require the Themosis addon in the project: the CLI loads the
project, so the native runner must be available as `ThemosisCliRunner`.

```sh
# Engine-independent source check.
themosis check theme/application.kdl

# Compile and map against the project's live engine.
themosis godot check --project . theme/application.kdl

# Materialize a native theme.
themosis godot build \
  --project . \
  --output res://theme/generated/application.tres \
  theme/application.kdl
```

The CLI accepts filesystem sources inside the project or `res://` sources, and
infers the project from the source or the current directory unless `--project`
is supplied. It accepts `--godot FILE`, then `THEMOSIS_GODOT_BINARY`, then
searches for `godot` or `godot4`. The executing engine must be Godot 4.5 or
newer; there is no upper-version selection table because its live `ClassDB`,
default `Theme`, and `ResourceLoader` decide availability. CI exercises the 4.5,
4.6, and 4.7 stable runtimes.

Use `--require-version 4.5.0` to reject any runtime whose numeric
`MAJOR.MINOR.PATCH` differs, or omit it to accept the 4.5 lower bound and later
compatible versions. Successful commands report the engine's display version and
commit hash. `--timeout SECONDS` changes the default 120-second limit.

Both source and output must resolve inside the canonical project directory.
Parent symlinks that escape the project are rejected before Godot starts, and
validation does not create output directories. Generation saves a temporary
sibling and replaces the requested output only after compilation, live mapping,
native construction, and `ResourceSaver` serialization succeed, so mapping and
version failures preserve an existing file. Use the same exact Godot version for
generation and project export when byte-for-byte reproducibility or exact
cross-version compatibility matters.

## Portable diagnostic codes

Portable planning reports every independent failure in deterministic
style/property order. Each rendered diagnostic includes its stable code.

| Code | Meaning |
| --- | --- |
| `TMS3001` | A compiled value category has no portable Godot theme-item mapping |
| `TMS3002` | A state changes the same native item as its base style |
| `TMS3003` | A numeric native item is not a valid whole pixel value |
| `TMS3004` | A resource reference is outside Godot's project resource namespace |

The engine-native builder returns symbolic codes such as
`unsupported_property`, `ambiguous_property`, and `incompatible_stylebox`.
The GDExtension preserves these codes and renders each native failure through
the same `error[CODE]: message` diagnostic envelope as portable failures.

A numeric item resolves to `constant` or `font_size` only when the target's
enumerated theme item list contains the property name. `Theme.has_font_size()`
and `Theme.has_font()` cannot be used for that check: they fall back to the
theme's default font size and default font and report true for any name on every
supported Godot version, measured on 4.5, 4.6 and 4.7.

Each generation reads referenced resources and their external dependencies afresh,
without replacing resources held by a previously generated Theme. A failed
rebuild therefore leaves the last valid preview intact.

The CLI supplies an inert runner scene, so checking or building a theme does not
require a configured application main scene. Runner diagnostics preserve source
byte spans as well as line/column locations and native item context.

Godot's `--editor --import` exit status alone is not a validation gate: run the
`ThemosisCheckRunner` validation before exporting imported themes, and treat a
nonzero exit as a stop-the-pipeline failure. Discovery skips hidden and linked
directories.

Addon materialization and profile configuration saves use the native persistence
service. A unique temporary sibling is removed on failure; successful saves
atomically replace the selected destination without using user-visible temporary
or backup names. Parent symlinks that escape the project are rejected.
