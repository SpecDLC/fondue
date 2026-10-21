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
const viewMs = performance.now() - t0;
console.log("200 views:", viewMs.toFixed(1), "ms");
// FONDUE-SITE-2: the timing is visible without opening the job log, and a slow engine fails the run.
const VIEW_LIMIT_MS = 1000;
if (process.env.GITHUB_STEP_SUMMARY) {
  const { appendFileSync } = await import("node:fs");
  appendFileSync(process.env.GITHUB_STEP_SUMMARY,
    `### FONDUE-SITE-2\n\n200 views: ${viewMs.toFixed(1)} ms (limit ${VIEW_LIMIT_MS} ms): ${viewMs < VIEW_LIMIT_MS ? "pass" : "FAIL"}\n`);
}
if (!(viewMs < VIEW_LIMIT_MS)) {
  console.error(`FONDUE-SITE-2 failed: 200 views took ${viewMs.toFixed(1)} ms, limit is under ${VIEW_LIMIT_MS} ms`);
  process.exit(1);
}
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

// FONDUE-ENG-8: adding an optional `why` to Step leaves every content id in the seed log unchanged
// unless a step's own fields change. SEED_IDS are pinned, not recomputed from a fresh hash on each run.
// MELT, SALT, and SEASON are hands-on (attend true) in the seed. That is an intentional content edit,
// so the béchamel and mac-and-cheese ids below were recorded again from the engine. The check still
// fails if any other id moves.
const SEED_IDS = [
  "c:bechamel@18ad3c7cd04e620b7b410bd0c4fe315a",
  "c:butter@42607be31efd3482d2794272b3de7881",
  "c:cooking-fat@50673d8e36dc328db4904b3874815fda",
  "c:cooking-liquid@c06f68a54ebc695714705d6200fcaed4",
  "c:flour@ea58dabf63944437710e013e9cf0f8ce",
  "c:greens@b3100503acd095ed3402497cadd63992",
  "c:mac-and-cheese@0643ba88be96bb42571dc652327aa229",
  "c:melting-cheese@4725542016e0ddad5b5fd847dee66b5e",
  "c:milk@b1023a85bc3b7251e79f0462ac328838",
  "c:nutmeg@72260f8340f847d8c857318ac534d0db",
  "c:olive-oil@52db6d297342210b5b84323841da9bc4",
  "c:onion@d710c11b075800140473d2ebe3b23b9b",
  "c:pasta@9b20a54f7e9b8e3070b2664b4f0f69cf",
  "c:plant-milk@9e0879f91c1c8f1cf546dce23bb427f1",
  "c:salad@7f0499e762cd4a13b1d08c6f2e7fd12c",
  "c:salt@41a8e08093d0fab4eecd21f4f314e473",
  "c:starch@18bea0c9c78850114a3ae56041411fb2",
  "c:vinegar@c9e6021ebd08a94c009f273263fce9a3",
  "g:sunday-dinner@1fe1a24168509dec479611c09816e7dd",
  "i:butter-salted@7a24401984e4dc75ecd3b541efdee98e",
  "i:butter-unsalted@5ab64e3705e3ca528757d9e900fa434b",
  "i:cheddar-block@c14a8dc1ef9ec999ef638c65000ef1c0",
  "i:cheddar-shredded@d7424a96b180540c6ca80a726679537f",
  "i:elbows@e2fa38183760ee4e06e211e209d033b1",
  "i:flour-ap@87e689abb2c65d96795afee3eea51561",
  "i:kerrygold-unsalted@2325e5ecc11d02792f26b1bdaf544f3c",
  "i:milk-whole@096d639760dac6761faecebde9575b6d",
  "i:mixed-greens@ed23e717f5b6b39393f6b7c2e2672fce",
  "i:nutmeg@a8896ba83fb68f8f19a82314008dabd8",
  "i:oat-milk@05d32da4091dc0dd1994a4bae3d051b4",
  "i:olive-oil-ev@cf6c64fab49b6aa7fd3034324bfee57c",
  "i:onion-yellow@55dabc91cf8071b4bdd82b8a99fd3983",
  "i:red-wine-vinegar@e07aaf4fd2bb80764588a6a70f406e55",
  "i:rice-flour@49c814c6315e211ac0bd2a68ece8bc79",
  "i:salt-kosher@8a84a81a29c0468163d0ebb757b7de58",
  "i:velveeta@d279c835396644c80ee234661a06e58b",
  "p:bechamel-classic@2444cabb74b9e0e8eb724f8e619ea673",
  "p:bechamel-classic@95c9d182cd88ff316c8a9446c4eaa42a",
  "p:bechamel-nana@d1bac0747fe536a7b3b5392d85725344",
  "p:bechamel-olive-oil@dc0fe0b8b2791c301b4563fc255b315b",
  "p:green-salad@0da578dbad41dd300c3b6c7b21421693",
  "p:mac-and-cheese-nana@ddcc016aa800b14b434aded0f11d64f4",
];
function seedIds(events) {
  call({ op: "load", events });
  const ids = new Set();
  for (const c of call({ op: "catalog" }).data) for (const t of c.tips) ids.add(c.node + "@" + t);
  for (const e of call({ op: "events" }).data) if (e.type === "head") ids.add(e.node + "@" + e.rev);
  return [...ids].sort();
}
function idDrift(events) {
  const got = new Set(seedIds(events)), want = new Set(SEED_IDS);
  return [...want].filter((i) => !got.has(i)).map((i) => "missing " + i).concat([...got].filter((i) => !want.has(i)).map((i) => "new " + i));
}
const seedEvents = JSON.parse(seed);
const drift = idDrift(seedEvents);
if (drift.length) throw new Error("ENG-8: content ids in the seed log changed:\n  " + drift.join("\n  "));
// The check must be able to fail: a seed where one step sets `why` is a different revision, and idDrift has to say so.
const broken = JSON.parse(seed);
const stepCommit = broken.find((e) => e.type === "commit" && e.body && e.body.steps && e.body.steps.length);
stepCommit.body.steps[0].why = "fixture: this step now sets why";
if (!idDrift(broken).length) throw new Error("ENG-8 check is blind: a step that sets why did not change any id");
call({ op: "load", events: seedEvents });
console.log("ENG-8: " + SEED_IDS.length + " seed ids match the pinned log; a step with why is detected");
