// Node smoke test for the WASM build: same ABI the browser uses.
import { readFileSync } from "node:fs";

const bytes = readFileSync(new URL("../target/wasm32-unknown-unknown/release/fondue_wasm.wasm", import.meta.url));
const { instance } = await WebAssembly.instantiate(bytes, {});
const x = instance.exports;
const enc = new TextEncoder(), dec = new TextDecoder();

function readOut(p) {
  const len = new DataView(x.memory.buffer).getUint32(p, true);
  const s = dec.decode(new Uint8Array(x.memory.buffer, p + 4, len));
  return [s, len];
}
function call(cmd) {
  const b = enc.encode(JSON.stringify(cmd));
  const p = x.fd_alloc(b.length);
  new Uint8Array(x.memory.buffer, p, b.length).set(b);
  const out = x.fd_call(p, b.length);
  x.fd_free(p, b.length);
  const [s, len] = readOut(out);
  x.fd_free(out, len + 4);
  return JSON.parse(s);
}

const [seed] = readOut(x.fd_seed());
console.log("exports:", Object.keys(x).join(", "));
console.log("load:", JSON.stringify(call({ op: "load", events: JSON.parse(seed) }).data));
const v = call({ op: "view", root: "g:sunday-dinner", cook: "patrick" });
if (!v.ok) throw new Error(v.error);
console.log("root:", v.data.tree.name, v.data.tree.level, v.data.tree.total_mins + " min");
console.log("lock lines:", v.data.lock.length);
for (const l of v.data.lock) console.log("  ", l.path.padEnd(42), l.node.padEnd(26), l.rev, l.via);
const pt = call({ op: "portal", concept: "c:bechamel", cook: "patrick" });
console.log("portal:", pt.data.map(e => `${e.name} [${e.check.status}] ★${e.outcome_avg.toFixed(1)}×${e.outcome_n}`).join(" | "));
const t0 = performance.now();
for (let i = 0; i < 200; i++) call({ op: "view", root: "g:sunday-dinner", cook: "patrick" });
console.log("200 views:", (performance.now() - t0).toFixed(1), "ms");
const bad = call({ op: "append", event: { type: "cook_log", cook: "x", node: "p:bechamel-nana", rev: "^", rating: 9 } });
console.log("bad event:", bad.ok, bad.error);
