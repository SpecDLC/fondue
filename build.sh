#!/usr/bin/env bash
# Build the core, run the tests, compile to WASM and assemble the demo page.
# Needs: a Rust toolchain. If the wasm32 target isn't installed (rustup can't
# download it), this builds std from source (needs the library sources in the
# toolchain's rust-src location) via -Zbuild-std.
set -euo pipefail
cd "$(dirname "$0")"

python3 seed/build_seed.py
cargo test -q -p fondue-core

if rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
  cargo build --release -p fondue-wasm --target wasm32-unknown-unknown
else
  RUSTC_BOOTSTRAP=1 cargo build --release -p fondue-wasm --target wasm32-unknown-unknown -Zbuild-std=std,panic_abort
fi

WASM=target/wasm32-unknown-unknown/release/fondue_wasm.wasm
node web/smoke.mjs
mkdir -p dist
python3 - "$WASM" <<'EOF'
import base64, pathlib, sys
wasm = pathlib.Path(sys.argv[1]).read_bytes()
tpl = pathlib.Path("web/index.template.html").read_text()
out = tpl.replace("__WASM_B64__", base64.b64encode(wasm).decode())
pathlib.Path("dist/fondue-demo.html").write_text(out)
print(f"dist/fondue-demo.html  ({len(out)//1024} KB, wasm {len(wasm)//1024} KB)")
EOF
