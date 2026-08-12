# HmosWorld fixture

The HmosWorld source is an optional external end-to-end fixture and is not
stored in this repository. From the arkdown repository root, create the local
checkout with:

```bash
git clone https://github.com/susns/HmosWorld tests/fixtures/hmosworld/HMOSWorld
```

The test project is then available at `HMOSWorld/Application` below this
directory. The checkout is ignored by Git. Keeping a normal directory here,
instead of a machine-specific symlink, makes the fixture usable on macOS,
Linux, and Windows.
