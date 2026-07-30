# Component-style KDL contract

A source is parsed as KDL 2.0 and contains exactly one `theme` root. KDL 1.0
fallback is not supported. JSON token files and imported KDL files are declared
explicitly; loading and path normalization are handled by the facade rather
than this parser.

```kdl
theme "dark" {
    tokens "tokens/dark.tokens.json"
    import "controls.kdl"

    style "PrimaryButton" target="Button" extends="BaseButton" {
        token "background" "color.primary"
        number "font-size" 16
        boolean "disabled" #false
        string "label" "Primary"
        resource "font" "res://fonts/ui.tres"

        state "hover" {
            token "background" "color.accent"
        }
    }
}
```

## Nodes

- `theme NAME` is the only root node.
- `tokens PATH` declares a DTCG-compatible JSON token source.
- `import PATH` declares another component-style KDL source.
- `style NAME target=CONTROL [extends=STYLE]` declares a component style. A custom name maps to a Godot theme type variation of `CONTROL`.
- `state NAME` contains explicit property overrides. There is no selector matching or cascade.

## Property values

Properties are explicit nodes with two arguments: property name and value.

- `boolean NAME VALUE` accepts `#true` or `#false`.
- `number NAME VALUE` accepts integer or decimal KDL numbers.
- `string NAME VALUE`
- `token NAME TOKEN_PATH` retains an unresolved design-token reference.
- `resource NAME REFERENCE` retains a non-empty, trimmed, backend-defined resource reference.
  The Godot backend accepts `res://` project resource paths; other backends may
  define a different reference namespace.

Duplicate style, state, and property names, inheritance, token existence, and
property type compatibility are semantic compiler responsibilities. Repeated
source imports are idempotent and are loaded once by the facade.

## Diagnostics

KDL parsing reports every independent failure in deterministic source traversal
order. Each rendered diagnostic includes its stable code.

| Code | Meaning |
| --- | --- |
| `TMS1001` | Invalid KDL 2 syntax |
| `TMS1002` | A parsed KDL declaration violates this format contract or a core invariant |
