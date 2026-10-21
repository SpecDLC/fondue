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
tpl = pathlib.Path("web/index.template.html").read_text(encoding="utf-8")
out = tpl.replace("__WASM_B64__", base64.b64encode(wasm).decode())
# The page must declare UTF-8 itself (within the first 1024 bytes), or browsers fall back to windows-1252.
head = out.encode("utf-8")[:1024].decode("utf-8", "ignore").lower()
assert head.startswith("<!doctype html>"), "page must start with <!doctype html>"
assert '<meta charset="utf-8">' in head, 'page needs <meta charset="utf-8"> in its first 1024 bytes'
pathlib.Path("dist/fondue-demo.html").write_text(out, encoding="utf-8")
print(f"dist/fondue-demo.html  ({len(out)//1024} KB, wasm {len(wasm)//1024} KB)")
EOF
# Workers static assets: serve the page as text/html; charset=utf-8 (belt and braces with the meta tag).
cp web/_headers dist/_headers
# Unknown paths and /favicon.ico are static files in dist/. wrangler.jsonc serves 404.html as the 404 body.
python3 - <<'PY'
import pathlib, struct
body = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Not found · Fondue</title>
</head>
<body>
<p>That page is not on this site.</p>
<p><a href="/">Back to Fondue</a></p>
</body>
</html>
"""
pathlib.Path("dist/404.html").write_text(body, encoding="utf-8")
w = h = 16
pixels = bytearray()
for y in range(h):
    for x in range(w):
        edge = x in (0, w - 1) or y in (0, h - 1)
        pixels += bytes((0xF8, 0xFB, 0xFB, 0xFF) if edge else (0x1B, 0x41, 0xC8, 0xFF))
mask = bytes(h * ((w + 31) // 32) * 4)
dib = struct.pack("<IiiHHIIiiII", 40, w, h * 2, 1, 32, 0, len(pixels) + len(mask), 0, 0, 0, 0)
image = dib + bytes(pixels) + mask
entry = struct.pack("<BBBBHHII", w, h, 0, 0, 1, 32, len(image), 22)
pathlib.Path("dist/favicon.ico").write_bytes(struct.pack("<HHH", 0, 1, 1) + entry + image)
print("dist/404.html and dist/favicon.ico")
PY

# Browser falsifiers for the site criteria (FONDUE-SITE-5 to SITE-11, plus ENG-8's smoke half): Playwright at 390 and 1440 px against the page just built.
# web/acceptance.mjs is not in release/release.manifest, so the public build has no such file and skips this. Without Playwright the step is skipped loudly.
# Exit 1 is a refutation and fails the build. Exit 2 (indeterminate or auxiliary) is reported and does not count as a pass.
if [ -f web/acceptance.mjs ]; then
  if node -e 'try { require.resolve("playwright") } catch (e) { require.resolve("playwright-core") }' 2>/dev/null; then
    rc=0; node web/acceptance.mjs || rc=$?
    if [ "$rc" -eq 1 ]; then echo "acceptance: a falsifier refuted the page" >&2; exit 1; fi
    if [ "$rc" -ne 0 ]; then echo "acceptance: indeterminate or auxiliary result (exit $rc), NOT a pass" >&2; fi
  else
    echo "acceptance: SKIPPED, Playwright is not installed (npm i playwright-core && npx playwright install chromium)" >&2
  fi
fi
