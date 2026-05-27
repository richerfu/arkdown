# SPEC-021: HAP Signing

# SPEC-022: hvigorw Compatibility Wrapper

---

## SPEC-021: HAP Signing

**Crate**: `arkdown-sign`
**Priority**: P1
**Blocked by**: SPEC-012 (packager)
**Blocks**: —

### Summary

Signs the unsigned `.hap`/`.hsp` output from the packager using the Huawei HAP signing tool
(`hap-sign-tool`). Signing credentials come from `build-profile.json5#app.signingConfigs`.

The HAP signing tool is a Java JAR distributed with the SDK. arkdown invokes it via
`java -jar hap-sign-tool.jar`.

### Signing Config

From `build-profile.json5`:

```json5
signingConfigs: [{
  name: "default",
  type: "HarmonyOS",
  material: {
    certpath: "path/to/cert.cer",
    keyAlias: "myKey",
    keyPassword: "encrypted_password",
    profile: "path/to/profile.p7b",
    signAlg: "SHA256withECDSA",
    storeFile: "path/to/keystore.p12",
    storePassword: "encrypted_password"
  }
}]
```

Passwords are AES-encrypted by DevEco Studio. arkdown must decrypt them using the same
key derivation as DevEco Studio (or require plaintext passwords via environment variable
`ARKDOWN_KEY_PASSWORD` and `ARKDOWN_STORE_PASSWORD` as a pragmatic alternative).

### Invocation

```bash
java -jar hap-sign-tool.jar sign-app \
    -keyAlias <keyAlias> \
    -signAlg SHA256withECDSA \
    -mode localSign \
    -appCertFile <certpath> \
    -profileFile <profile> \
    -inFile <unsigned.hap> \
    -outFile <signed.hap> \
    -keystoreFile <storeFile> \
    -keystorePwd <storePassword> \
    -keyPwd <keyPassword>
```

### Acceptance Criteria

- Signed `.hap` is accepted by `hdc install` on a real OHOS device
- Build succeeds without signing config (produces unsigned `.hap` with a warning)
- Clear error when `java` is not in PATH

---

## SPEC-022: hvigorw Compatibility Wrapper

**Crate**: `arkdown-cli` (extension to SPEC-013)
**Priority**: P1
**Blocked by**: SPEC-013

### Summary

Provides a drop-in `hvigorw` shell script that translates hvigor commands to arkdown. Existing
CI pipelines that call `./hvigorw assembleHap` work without modification.

### hvigorw Shell Script

Write this file to `./hvigorw` in the project root (or as a template users install):

```bash
#!/bin/bash
# hvigorw — drop-in replacement for arkdown
# See meta/design/compat.md for the full command mapping.

ARKDOWN_BIN="${ARKDOWN_HOME:-$(which arkdown 2>/dev/null)}"
if [ -z "$ARKDOWN_BIN" ]; then
    echo "ERROR: arkdown not found. Install arkdown or set ARKDOWN_HOME." >&2
    exit 2
fi

CMD="${1:-default}"; shift 2>/dev/null || true
ARGS=()
MODE="debug"
MODULE=""
PRODUCT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --mode)   MODE="$2";    shift 2 ;;
        -m)       MODULE="$2";  shift 2 ;;
        --product) PRODUCT="$2"; shift 2 ;;
        -p)       ARGS+=("-p" "$2"); shift 2 ;;
        *)        ARGS+=("$1"); shift ;;
    esac
done

TARGET_ARGS=()
[[ -n "$MODULE" ]]  && TARGET_ARGS+=("--module"  "$MODULE")
[[ -n "$PRODUCT" ]] && TARGET_ARGS+=("--product" "$PRODUCT")

case "$CMD" in
    assembleHap)  exec "$ARKDOWN_BIN" build --target hap --mode "$MODE" "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    assembleHsp)  exec "$ARKDOWN_BIN" build --target hsp --mode "$MODE" "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    assembleHar)  exec "$ARKDOWN_BIN" build --target har --mode "$MODE" "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    assembleApp)  exec "$ARKDOWN_BIN" build --target app --mode "$MODE" "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    clean)        exec "$ARKDOWN_BIN" clean "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    test)         exec "$ARKDOWN_BIN" test  "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    default|"")   exec "$ARKDOWN_BIN" build "${TARGET_ARGS[@]}" "${ARGS[@]}" ;;
    *)
        echo "ERROR: Unknown hvigorw command: $CMD" >&2
        echo "arkdown supports: assembleHap, assembleHsp, assembleHar, assembleApp, clean, test" >&2
        exit 2
        ;;
esac
```

### `arkdown init` Command (optional)

Add `arkdown init --compat` which writes the `hvigorw` script to the project root and
makes it executable.

### Unsupported hvigor Features

Document clearly that these hvigor features are NOT supported:

- Custom `hvigorfile.ts` task definitions (Gradle plugin extension points)
- Java daemon (`hvigor-java-daemon.jar`) — arkdown is a single native binary
- `--daemon` / `--no-daemon` flags (no-op)
- Gradle build cache compatibility

### Acceptance Criteria

- `./hvigorw assembleHap` in `MyApplication` produces the same `.hap` as `arkdown build --target hap`
- `./hvigorw assembleHap --mode release` passes `--mode release` to arkdown
- `-m entry` scopes build to the `entry` module

---

## Design References

- `meta/design/compat.md` — complete command mapping, migration guide
