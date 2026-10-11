#!/usr/bin/env bash

set -euo pipefail

PACKAGE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$PACKAGE_DIR/../.." && pwd)"
REGISTRY="https://registry.npmjs.org/"
DRY_RUN=0

if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=1
elif [[ $# -ne 0 ]]; then
  echo "Usage: pnpm run publish:local [--dry-run]" >&2
  exit 2
fi

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "Local release must run on an arm64 macOS host." >&2
  exit 1
fi

for command_name in cargo cargo-zigbuild cargo-xwin file git node npm pnpm rustup tar zip; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Required command is missing: $command_name" >&2
    exit 1
  fi
done

if ! command -v clang-cl >/dev/null 2>&1 && command -v brew >/dev/null 2>&1; then
  export PATH="$(brew --prefix llvm)/bin:$PATH"
fi
if ! command -v lld-link >/dev/null 2>&1 && command -v brew >/dev/null 2>&1; then
  export PATH="$(brew --prefix lld)/bin:$PATH"
fi
for command_name in clang-cl lld-link llvm-ar; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Required LLVM command is missing: $command_name (install Homebrew llvm and lld)." >&2
    exit 1
  fi
done

if [[ $DRY_RUN -eq 0 && -z "${NODE_AUTH_TOKEN:-}" ]]; then
  echo "NODE_AUTH_TOKEN is required for publishing." >&2
  exit 1
fi
if [[ $DRY_RUN -eq 1 && -z "${NODE_AUTH_TOKEN:-}" ]]; then
  export NODE_AUTH_TOKEN="arkdown-dry-run"
fi
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/arkdown-publish.XXXXXX")"
PACKAGE_JSON_BACKUP="$TEMP_DIR/package.json"
README_WAS_SYMLINK=0
PACKAGE_JSON_BACKED_UP=0

cleanup() {
  if [[ $PACKAGE_JSON_BACKED_UP -eq 1 ]]; then
    cp "$PACKAGE_JSON_BACKUP" "$PACKAGE_DIR/package.json"
  fi
  if [[ $README_WAS_SYMLINK -eq 1 ]]; then
    rm -f "$PACKAGE_DIR/README.md"
    ln -s ../../README.md "$PACKAGE_DIR/README.md"
  fi
  rm -rf "$TEMP_DIR"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

printf '%s\n' \
  "registry=$REGISTRY" \
  "@ohos-rs:registry=$REGISTRY" \
  '//registry.npmjs.org/:_authToken=${NODE_AUTH_TOKEN}' \
  >"$TEMP_DIR/npmrc"
export NPM_CONFIG_USERCONFIG="$TEMP_DIR/npmrc"
export NPM_CONFIG_CACHE="$TEMP_DIR/npm-cache"
export NPM_CONFIG_PROVENANCE=false
unset npm_config_verify_deps_before_run NPM_CONFIG_VERIFY_DEPS_BEFORE_RUN

if [[ $DRY_RUN -eq 0 ]]; then
  echo "Checking npm token..."
  npm whoami --registry "$REGISTRY" >/dev/null
fi

echo "Verifying the pinned compiler crates..."
node "$PACKAGE_DIR/scripts/compiler-source.mjs" --verify

export XWIN_CACHE_DIR="${XWIN_CACHE_DIR:-$PACKAGE_DIR/.cache/cargo-xwin}"
mkdir -p "$XWIN_CACHE_DIR"

echo "Installing locked JavaScript dependencies..."
pnpm --dir "$PACKAGE_DIR" install --frozen-lockfile

echo "Installing Rust compilation targets..."
rustup target add \
  aarch64-apple-darwin \
  x86_64-unknown-linux-gnu \
  x86_64-pc-windows-msvc

echo "Clearing Rust build cache and previous package outputs..."
cd "$REPO_ROOT"
cargo clean
rm -rf \
  "$PACKAGE_DIR/artifacts" \
  "$PACKAGE_DIR/npm" \
  "$PACKAGE_DIR/arkdown.darwin-arm64.node" \
  "$PACKAGE_DIR/arkdown.linux-x64-gnu.node" \
  "$PACKAGE_DIR/arkdown.win32-x64-msvc.node"

echo "Building macOS arm64 package..."
pnpm --dir "$PACKAGE_DIR" run build --target aarch64-apple-darwin

echo "Building Linux x64 GNU package with cargo-zigbuild..."
pnpm --dir "$PACKAGE_DIR" run build --target x86_64-unknown-linux-gnu --cross-compile

echo "Building Windows x64 MSVC package with cargo-xwin..."
cargo xwin build \
  --locked \
  --release \
  --target x86_64-pc-windows-msvc \
  -p arkdown-napi \
  --lib
cp \
  "$REPO_ROOT/target/x86_64-pc-windows-msvc/release/arkdown_napi.dll" \
  "$PACKAGE_DIR/arkdown.win32-x64-msvc.node"
pnpm --dir "$PACKAGE_DIR" exec prettier index.d.ts --write

MAC_KIND="$(file -b "$PACKAGE_DIR/arkdown.darwin-arm64.node")"
LINUX_KIND="$(file -b "$PACKAGE_DIR/arkdown.linux-x64-gnu.node")"
WINDOWS_KIND="$(file -b "$PACKAGE_DIR/arkdown.win32-x64-msvc.node")"
if [[ "$MAC_KIND" != *"Mach-O 64-bit"* || "$MAC_KIND" != *"arm64"* ]]; then
  echo "Invalid macOS artifact: $MAC_KIND" >&2
  exit 1
fi
if [[ "$LINUX_KIND" != *"ELF 64-bit"* || "$LINUX_KIND" != *"x86-64"* ]]; then
  echo "Invalid Linux artifact: $LINUX_KIND" >&2
  exit 1
fi
if [[ "$WINDOWS_KIND" != *"PE32+"* || "$WINDOWS_KIND" != *"x86-64"* ]]; then
  echo "Invalid Windows artifact: $WINDOWS_KIND" >&2
  exit 1
fi
printf 'Validated native artifacts:\n  macOS:  %s\n  Linux:  %s\n  Windows: %s\n' \
  "$MAC_KIND" "$LINUX_KIND" "$WINDOWS_KIND"

echo "Running package tests with the native macOS binding..."
pnpm --dir "$PACKAGE_DIR" test
pnpm --dir "$PACKAGE_DIR" smoke

echo "Assembling platform packages..."
mkdir -p \
  "$PACKAGE_DIR/artifacts/bindings-aarch64-apple-darwin" \
  "$PACKAGE_DIR/artifacts/bindings-x86_64-unknown-linux-gnu" \
  "$PACKAGE_DIR/artifacts/bindings-x86_64-pc-windows-msvc"
cp "$PACKAGE_DIR/arkdown.darwin-arm64.node" \
  "$PACKAGE_DIR/artifacts/bindings-aarch64-apple-darwin/arkdown.darwin-arm64.node"
cp "$PACKAGE_DIR/arkdown.linux-x64-gnu.node" \
  "$PACKAGE_DIR/artifacts/bindings-x86_64-unknown-linux-gnu/arkdown.linux-x64-gnu.node"
cp "$PACKAGE_DIR/arkdown.win32-x64-msvc.node" \
  "$PACKAGE_DIR/artifacts/bindings-x86_64-pc-windows-msvc/arkdown.win32-x64-msvc.node"
pnpm --dir "$PACKAGE_DIR" exec napi create-npm-dirs
pnpm --dir "$PACKAGE_DIR" artifacts

cp "$PACKAGE_DIR/package.json" "$PACKAGE_JSON_BACKUP"
PACKAGE_JSON_BACKED_UP=1
if [[ -L "$PACKAGE_DIR/README.md" ]]; then
  README_WAS_SYMLINK=1
  rm "$PACKAGE_DIR/README.md"
  cp "$REPO_ROOT/README.md" "$PACKAGE_DIR/README.md"
fi

cd "$PACKAGE_DIR"
pnpm exec napi pre-publish --no-gh-release -t npm --skip-optional-publish
node "$PACKAGE_DIR/scripts/compiler-source.mjs" --package "$PACKAGE_DIR/npm"

VERSION="$(node -p "require('./package.json').version")"
if [[ "$VERSION" == *-* ]]; then
  DIST_TAG="${VERSION#*-}"
  DIST_TAG="${DIST_TAG%%.*}"
else
  DIST_TAG="latest"
fi

CHILD_PACKAGE_JSONS=(
  "$PACKAGE_DIR/npm/darwin-arm64/package.json"
  "$PACKAGE_DIR/npm/linux-x64-gnu/package.json"
  "$PACKAGE_DIR/npm/win32-x64-msvc/package.json"
)

if [[ $DRY_RUN -eq 1 ]]; then
  echo "Validating npm tarballs without publishing..."
  for child_package_json in "${CHILD_PACKAGE_JSONS[@]}"; do
    npm pack "$(dirname "$child_package_json")" --dry-run --ignore-scripts --json >/dev/null
  done
  npm pack . --dry-run --ignore-scripts --json >/dev/null
  echo "Dry run completed for version $VERSION with dist-tag $DIST_TAG."
  exit 0
fi

echo "Publishing version $VERSION with dist-tag $DIST_TAG..."
for child_package_json in "${CHILD_PACKAGE_JSONS[@]}"; do
  child_directory="$(dirname "$child_package_json")"
  child_name="$(node -e "console.log(require(process.argv[1]).name)" "$child_package_json")"
  if npm view "$child_name@$VERSION" version --registry "$REGISTRY" >/dev/null 2>&1; then
    echo "Skipping existing package $child_name@$VERSION"
  else
    npm publish "$child_directory" \
      --ignore-scripts \
      --access public \
      --tag "$DIST_TAG" \
      --provenance=false \
      --registry "$REGISTRY"
  fi
done

MAIN_NAME="$(node -p "require('./package.json').name")"
if npm view "$MAIN_NAME@$VERSION" version --registry "$REGISTRY" >/dev/null 2>&1; then
  echo "Skipping existing package $MAIN_NAME@$VERSION"
else
  npm publish . \
    --ignore-scripts \
    --access public \
    --tag "$DIST_TAG" \
    --provenance=false \
    --registry "$REGISTRY"
fi

echo "Published $MAIN_NAME@$VERSION and its three platform packages."
