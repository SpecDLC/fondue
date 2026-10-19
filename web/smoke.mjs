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

// Loading a recipe from JSON: a good one is applied, a bad one names its field and appends nothing.
const nEvents = () => call({ op: "events" }).data.length;
const soup = { id: "smoke-soup", title: "Smoke Soup", serves: "2",
  slots: [{ id: "fat", role: "fat", concept: "c:cooking-fat", qty: "1 Tbsp", bind: "i:butter-unsalted" }],
  steps: [{ verb: "SWEAT", text: "Onion in the fat.", mins: 5, attend: false, uses: ["fat"], cue: "soft" }] };
const before = nEvents();
const good = call({ op: "import", recipe: soup, cook: "patrick" });
if (!good.ok || nEvents() <= before) throw new Error("import refused a good recipe: " + good.error);
const sheet = call({ op: "view", root: "p:smoke-soup", cook: "patrick" }).data.tree;
if (sheet.steps.length !== 1 || sheet.slots[0].child.node !== "i:butter-unsalted") throw new Error("imported recipe lost a step or slot");
const after = nEvents();
const noVerb = call({ op: "import", recipe: { ...soup, id: "smoke-bad", steps: [{ text: "x", mins: 1, attend: false, uses: [], cue: "" }] }, cook: "patrick" });
if (noVerb.ok || !noVerb.error.includes("steps[0].verb") || nEvents() !== after) throw new Error("bad import was not refused cleanly: " + JSON.stringify(noVerb));
console.log("import:", good.data.node, "applied;", noVerb.error);
