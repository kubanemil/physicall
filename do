#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

WASM_CRATE=crates/physicall-web

help() {
  cat <<'USAGE'
Usage: ./do <command>

  test    run every test (GPU tests skip without an adapter)
  check   rustfmt check, clippy for native and wasm32, TypeScript check
  fmt     format the Rust code
  dev     build the wasm package and start the Vite dev server
  build   build the release wasm package into crates/physicall-web/pkg
  web     build the static site into web/dist
  help    show this message
USAGE
}

npm_install() {
  if [[ ! -d web/node_modules ]]; then
    (cd web && npm ci)
  fi
}

case "${1:-help}" in
  test)
    cargo test --workspace -- --show-output
    ;;
  check)
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy --workspace --lib --target wasm32-unknown-unknown -- -D warnings
    if [[ -d "$WASM_CRATE/pkg" ]]; then
      npm_install
      (cd web && npx tsc --noEmit)
    fi
    ;;
  fmt)
    cargo fmt --all
    ;;
  dev)
    wasm-pack build "$WASM_CRATE" --dev --target web
    npm_install
    cd web && npm run dev
    ;;
  build)
    wasm-pack build "$WASM_CRATE" --release --target web
    ;;
  web)
    "$0" build
    npm_install
    cd web && npm run build
    ;;
  help | -h | --help)
    help
    ;;
  *)
    echo "unknown command: $1" >&2
    help >&2
    exit 1
    ;;
esac
