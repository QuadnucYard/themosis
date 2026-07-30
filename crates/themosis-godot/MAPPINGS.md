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

The Godot integration is split across two crates and a shared native builder:

- `themosis-godot` validates portable constraints and produces a serializable build plan without depending on `godot-rust`. It also supplies the engine-native GDScript builder.
- Both consumers pass that same plan to that same builder. The CLI runner starts a selected headless Godot executable, checks its version, and saves the returned `Theme` as `.tres` through `ResourceSaver`. The plugin executes the builder in the engine that loaded the GDExtension and returns the live `Theme` object; its editor importer never shells out to the CLI.
- `themosis-godot-plugin` uses `godot` for GDExtension, project I/O, and live Godot objects; it does not maintain a second mapping implementation.

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
**Reimport all** expose the same operation on demand. **Materialize** is the
explicit alternative when a stable, visible `.tres` output is required.

Generate a native theme without loading the plugin:

```sh
themosis build --target godot \
  --godot godot \
  --project . \
  --output res://theme/generated/application.tres \
  theme/application.kdl
```

The CLI accepts `--godot FILE`, then `THEMOSIS_GODOT_BINARY`, then searches for `godot` or `godot4`. The executing engine must be Godot 4.5 or newer; there is no upper-version selection table because its live `ClassDB`, default `Theme`, and `ResourceLoader` decide availability. CI exercises the minimum 4.5 runtime and a newer stable runtime.

Use `--require-godot-version 4.5.0` to reject any runtime whose numeric `MAJOR.MINOR.PATCH` differs, or omit it to accept the 4.5 lower bound and later compatible versions. Successful commands report the engine's display version and commit hash. `--godot-timeout SECONDS` changes the default 120-second limit.

Output must resolve inside the canonical project directory. Parent symlinks that escape the project are rejected before Godot starts, and validation does not create output directories. Generation saves a temporary sibling and replaces the requested output only after compilation, live mapping, native construction, and `ResourceSaver` serialization succeed, so mapping and version failures preserve an existing file. Use the same exact Godot version for generation and project export when byte-for-byte reproducibility or exact cross-version compatibility matters.

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
