# arkdown

Node.js bindings and npm CLI for arkdown, the OpenHarmony build tool backed by
the Rust workspace in this repository.

## Usage

```js
const { build } = require('arkdown')

const result = await build({
  project: './tests/fixtures/minimal-hap',
  target: 'hap',
  mode: 'debug',
})

console.log(result.artifacts)
```

The package also exposes a CLI wrapper:

```sh
arkdown build --project ./tests/fixtures/minimal-hap --target hap --mode debug
```

The native addon is produced with napi-rs:

```sh
pnpm build
```
