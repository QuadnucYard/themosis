# Godot integration decisions

Status: implemented. This record explains the selected integration and the
alternatives that remain available.

## Primary workflow: native importer

The addon registers an `EditorImportPlugin` for `.tms` roots. Each root is one
concrete theme composition and imports as a native Godot `Theme`:

```text
res://theme/light.tms -> res://.godot/imported/...tres
res://theme/dark.tms  -> res://.godot/imported/...tres
```

Users reference the source paths. Godot owns the cache path, import metadata,
export discovery, and resource UIDs. The importer calls
`ThemosisThemeGenerator` in the GDExtension directly and never shells out to
the CLI.

One root per concrete theme was selected over a multi-output root because it
matches Godot's one-source/one-resource importer model, gives every theme its
own UID, and makes per-theme invalidation deterministic. Shared `.kdl` files
are wrapper-free fragments; roots typically differ only in palette token JSON.

## Dependency invalidation

The facade and extension report exact dependencies discovered on both success
and failure. A successful imported Theme stores a deterministic graph
fingerprint. After the editor filesystem scan, the plugin compares persisted
and current fingerprints and reimports only stale roots. While the editor is
open, MD5 snapshots and a 350 ms debounce handle subsequent changes.

This persists invalidation across editor restarts without importing or
hijacking arbitrary JSON files. Shared source changes rebuild every affected
root; theme-specific changes rebuild only that theme.

## Editor experience

The dock discovers roots recursively and offers:

- selected and global reimport actions;
- a native preview;
- importing, stale, up-to-date, and failed status;
- structured diagnostics with stable codes and clickable paths;
- selected and global materialization actions.

A toolbar button exposes global reimport without opening the dock. Importer
output has no configurable path because Godot owns it.

## Secondary workflow: materialization

Materialization explicitly saves a visible stable `.tres`, chosen with a Godot
resource `FileDialog`. It is useful for source control, external tooling, or
pipelines that require a fixed path. Writes are confined to `res://`, reject
ambiguous path segments, and preserve the prior valid output on failure.

Optional `themosis.godot.json` profiles remain deterministic presets for the
headless addon script:

```sh
godot --headless --path . \
  --script res://addons/themosis/build.gd -- --all
```

The headless script also calls the GDExtension, not the CLI.

## Other integration choices

- `ThemosisThemeGenerator.generate_result()` is retained for mod and
  user-authored runtime themes. It is not the default because it adds startup
  compilation and requires shipping raw sources.
- The standalone Rust CLI remains useful outside the addon. It launches a
  selected Godot executable and uses the same portable plan/native builder,
  but no editor workflow depends on it.
- An `EditorExportPlugin` hook was rejected. Imported assets already integrate
  with export, while hidden export-time mutation would create a second build
  authority.
- Importing every KDL and token JSON dependency as a separate asset was
  rejected. It would either hijack all `.json` files or require unnatural token
  extensions; persisted root fingerprints provide precise invalidation without
  that cost.

## Source and backend defects addressed

Two format/backend limitations found during the audit were corrected:

1. Imported KDL modules can now inherit the root theme name and omit a repeated
   `theme` wrapper. Legacy wrapped imports remain accepted and name-checked.
2. A style whose name equals its target now writes native Godot defaults. Other
   styles remain type variations. Ordinary controls no longer require a
   variation assignment solely to receive theme styling.

The build-plan schema was advanced to version 2 for the default-type mapping.

## Acceptance coverage

Tests cover two independent imported roots, real dark/light switching, shared
and theme-specific dependency graphs, default types and named variations,
structured failures, confined paths, previous-output preservation, headless
materialization, editor loading, and packaged-addon completeness.
