# Fondue

> A recipe graph for engineering-minded cooks.

**Live:** https://fondue.specdlc.com · **Part of:** [SpecDLC](https://specdlc.com), spec-driven delivery with acceptance criteria as the contract.

Butter is an ingredient, béchamel is a component, mac and cheese is a dish, Sunday dinner is a meal. In Fondue they are all the same kind of node, and the zoom level comes from how they nest. Recipes are data, and every view (a printed sheet, a live step on a tablet, a lockfile) is a projection of that data.

This is **prototype 0.2**: a Rust engine compiled to WebAssembly, and a single-page demo that resolves a whole dinner in about 0.6 ms in the browser. It exists to prove the load-bearing ideas, not to be feature-rich.

## What it does

- **Slots and bindings.** A recipe declares slots: a role, a concept and an amount. A slot is bound, in order, by the cook's ask-once choice, the pantry, the recipe author's default, then a canon's suggestion. The pantry only replaces a default of the same kind: keeping butter doesn't turn an olive-oil roux into butter.
- **Canons and contracts.** Cooks subscribe to curated sets of rules (`must` and `should`), in priority order, with a dial: off, advise or enforce. A bound child answers to the rules of everything it *is*, not just what the slot asked for.
- **Facts flow up.** `"fat": "@slot:fat.fat"` lets a béchamel's contract see that its roux is olive oil.
- **Variants** are recipe-local deltas triggered by a binding. Velveeta drops the béchamel; salted butter drops the added salt.
- **Everything is an event.** Content-addressed commits (canonical JSON plus the schema tag `fondue/1`), heads per scope (`main`, `canon:x`, `cook:y`), canons, profiles, cook logs, labels and publishes. State is a fold of the log, and every view is a projection.

## The contract: acceptance criteria

The demo is built against written, falsifiable acceptance criteria in [`ac/`](ac/). They are **drafts** until ratified. Each card states a claim, the interface a check may use, the counterexample that refutes it, and how we know it holds.

| ID | Claim | Status |
|---|---|---|
| `FONDUE-ENG-1` … `FONDUE-ENG-7` | Engine behavior, each backed by named tests in `crates/fondue-core/tests` | draft |
| `FONDUE-SITE-1` … `FONDUE-SITE-4` | Build, speed, hosting and privacy of the demo page | draft |

## Layout

```
ac/                  acceptance criteria (draft)
crates/fondue-core   Rust: event log → projections (resolve, portal, lockfile, diff)
crates/fondue-wasm   C ABI shell: JSON in, JSON out. No wasm-bindgen.
seed/                build_seed.py writes events.json, the sample log: butter → béchamel → mac & cheese → dinner
web/                 Demo page template and a Node smoke test
build.sh             Seed, test, compile to WASM, smoke-test, assemble dist/fondue-demo.html
```

## Run it locally

```sh
./build.sh                         # needs rustup, Python 3 and Node; rust-toolchain.toml pins Rust and adds wasm32
open dist/fondue-demo.html         # self-contained page, engine inlined as WASM
cargo test -p fondue-core          # scenario tests
```

Without rustup's `wasm32-unknown-unknown` target, `build.sh` falls back to building std from source with `-Zbuild-std`, which needs the matching `rust-src`.

## Not in scope yet

Lanes and scheduling (children's times currently add in sequence, so mac and cheese claims 37 minutes while the pasta actually boils during the béchamel), signatures and a full event envelope, resolving *from* a lockfile, FoodData Central import, and print layouts generated from the data.

## How this repo is published

This repository holds the polished, reviewed parts of the project: the acceptance criteria and the source of the engine and demo page. Each change arrives as a release pull request that has passed a secret scan and a privacy scan and is approved before merge. Merging to `main` deploys https://fondue.specdlc.com on Cloudflare.

Issues and suggestions are welcome. Pull requests are read and, when accepted, folded into the next release.

## License

MIT. See [LICENSE](LICENSE).
