# Token JSON contract

`themosis-tokens` accepts strict JSON documents following a deliberately small subset of the Design Tokens Community Group format.

## Structure

- A JSON object without `$value` is a group.
- A JSON object with `$value` is a token.
- `$type` on a group is inherited by descendant tokens unless a nested group or token overrides it.
- Every token must have an effective `$type`.
- `$description`, `$extensions`, and `$deprecated` are accepted as metadata and currently ignored.
- Other properties beginning with `$` are rejected.
- Group and token names must be non-empty and cannot contain `.`, `{`, or `}`.

## Supported types

| `$type` | `$value` |
| --- | --- |
| `boolean` | JSON boolean |
| `number` | finite JSON number |
| `string` | JSON string |
| `dimension` | `{ "value": number, "unit": "px" | "rem" }` |
| `color` | `{ "colorSpace": "srgb", "components": [r, g, b], "alpha": a }` with components in `0..=1` |

An exact string of the form `{group.token}` is decoded as an unresolved alias for any token type. Alias existence, cycles, and type compatibility are compiler responsibilities.

No other DTCG types, color spaces, component value forms, or composite extensions are accepted in version one.
