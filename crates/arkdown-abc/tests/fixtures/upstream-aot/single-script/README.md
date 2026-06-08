# Fixture: single-script

Source: `main.js` — a simple script with variable declarations.

## Upstream command

```bash
es2abc --output single-script.abc main.js
```

## Golden files

- `single-script.program-dump` — normalized pandasm Program dump
- `single-script.abc.hash` — SHA-256 of the output ABC

## Oracle mode

With `ARKDOWN_UPSTREAM_ES2ABC=/path/to/es2abc`:

```bash
cargo test -p arkdown-abc upstream_aot_oracle
```
