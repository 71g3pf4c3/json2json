# json2json

fuck json2dir, let's do json2json.

A command-line tool that reads JSON and writes the same JSON back out.
Key order, duplicate keys and number lexical forms survive untouched —
only the whitespace changes. Zero dependencies. One binary.

## Why not json2dir

[json2dir](https://github.com/alurm/json2dir) stores JSON as a filesystem
tree: objects become directories, keys become filenames, scalars become
file contents. Sounds clever. In practice it's an exercise in fighting
your own storage layer:

**The filesystem is a hostile data model for JSON.** Any string is a legal
JSON key; not any string is a legal filename. `/`, `\0`, 255-byte limits,
`.` and `..`, trailing spaces — json2dir needs escaping conventions just to
store your data, and every convention is a bug you get to discover later.
On case-insensitive filesystems (macOS, Windows) `{"Key": 1, "key": 2}`
silently collides into one entry. That's data loss you can't detect until
restore time.

**It's expensive.** An object with 10,000 keys becomes 10,000 inodes and
10,000 filesystem blocks — 4 KiB each — holding values that average a few
bytes. A small JSON file routinely blossoms into megabytes on disk.
Reading it back is one `stat()` per node. Writing it is one
`open()/write()/close()` per node. That's O(keys) syscalls to accomplish
what a single `read()` already does.

**It's ambiguous.** A file containing the bytes `null`: is that JSON `null`
or the string `"null"`? A convention answers that question, and conventions
drift. Meanwhile the file's mode, uid and mtime are not part of your data,
but are now part of your snapshot — diffs are noise, merges are hell.

**And it's circular by design.** The only way to actually consume the tree
is to convert it back to JSON. So the full json2dir lifecycle is:

```
json → dir → json
```

json2json does that in one step, losslessly, and the data never leaves the
format it was already in.

## Guarantees

A round-trip through json2json is semantically identical input:

| Property        | Behavior                                                            |
| --------------- | ------------------------------------------------------------------- |
| Key order       | preserved exactly, no sorting                                        |
| Duplicate keys  | preserved — `{"b":1,"b":2}` stays two entries                          |
| Numbers         | verbatim lexical form: `1e2` never becomes `100.0`, `9007199254740993` never passes through an `f64` |
| Strings         | decoded and re-encoded with minimal escapes; contents identical      |
| Unicode         | raw UTF-8 passthrough, no `\uXXXX` inflation                          |
| Whitespace      | normalized — the only thing that changes                              |
| Input strictness | RFC 8259, errors reported with `line:column`                         |
| Nesting         | up to 512 levels; deeper input is rejected, not crashed               |

The transformation is idempotent: `f(f(x)) == f(x)`.

## Usage

```
json2json [OPTIONS] [INPUT] [OUTPUT]
```

`INPUT` and `OUTPUT` default to stdin/stdout; `-` works explicitly.

```console
$ cat ugly.json | json2json
$ json2json in.json out.json
$ json2json --compact - < in.json
$ json2json --indent 4 in.json
```

Options:

- `-c`, `--compact` — one line, no whitespace
- `-i`, `--indent <N>` — spaces per level for pretty output (default: 2)

Exit codes: `0` success, `1` bad JSON, `2` usage or I/O error.

## Install

### Nix

```console
$ nix run github:71g3pf4c3/json2json -- --help
$ nix build github:71g3pf4c3/json2json
```

Or via the flake's overlay:

```nix
inputs.json2json.url = "github:71g3pf4c3/json2json";
```

On NixOS:

```nix
{
  inputs.json2json.url = "github:71g3pf4c3/json2json";

  # in your NixOS host:
  imports = [ inputs.json2json.nixosModules.default ];
  programs.json2json.enable = true;
}
```

With home-manager:

```nix
{
  imports = [ inputs.json2json.homeManagerModules.default ];
  programs.json2json.enable = true;
}
```

The module exposes two options: `enable` (off by default) and `package`
(defaults to the flake's build for your system, so no overlay is required —
override it if you want a pinned or patched build).

### Cargo

```console
$ cargo install --git https://github.com/71g3pf4c3/json2json
```

## Library

The binary is a thin wrapper over a small library:

```rust
use json2json::{parse, to_string};

let value = parse(r#"{"a": [1e2, "x", null]}"#)?;
let pretty = to_string(&value);
assert_eq!(pretty, "{\n  \"a\": [\n    1e2,\n    \"x\",\n    null\n  ]\n}");
```

## Development

```console
$ nix develop -c cargo test
$ nix build
```

Layout:

- `src/parser.rs` — strict RFC 8259 recursive-descent parser
- `src/value.rs` — order-preserving data model (`Value`, `Number`, `Object`)
- `src/serializer.rs` — pretty/compact writer
- `src/error.rs` — positional parse errors

## License

GPL-3.0-or-later.
