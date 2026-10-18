# FONDUE-SITE-1 (draft)

**Claim.** `./build.sh` on a clean checkout with the pinned toolchain seeds the data, runs the tests, compiles the engine to WASM, smoke-tests it, and writes one self-contained page with the engine inlined.

**Interface.** The public repository at a release commit and CI logs.

**Forbids.** A build that needs files not in the public repository, or a page that loads the engine from a separate URL.

**Done when.** The Verify workflow runs `./build.sh` green on the release commit.
