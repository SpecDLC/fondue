"""Builds seed/events.json: butter → béchamel → mac and cheese → Sunday dinner.

Run: python3 seed/build_seed.py
The output is an append-only event log; the engine replays it to build state.
"""
import json, pathlib

ev = []
def commit(node, author, body, msg="", parent=None, forked_from=None):
    e = {"type": "commit", "node": node, "author": author, "msg": msg, "body": body}
    if parent: e["parent"] = parent
    if forked_from: e["forked_from"] = {"node": forked_from}
    ev.append(e)

def concept(cid, name, extends=None, note=""):
    b = {"kind": "concept", "name": name, "note": note}
    if extends: b["extends"] = extends
    commit(cid, "curated-core", b)

def item(iid, name, implements, facts, brand=None, source=None, author="curated-core"):
    b = {"kind": "item", "name": name, "implements": implements, "facts": facts}
    if brand: b["brand"] = brand
    if source: b["source"] = source
    commit(iid, author, b)

def slot(sid, role, concept, qty, bind=None):
    s = {"id": sid, "role": role, "concept": concept, "qty": qty}
    if bind: s["bind"] = {"node": bind}
    return s

def step(verb, text, mins, uses=(), attend=False, cue=""):
    return {"verb": verb, "text": text, "mins": mins, "uses": list(uses), "attend": attend, "cue": cue}

# ---------- concepts (the curated core) ----------
concept("c:cooking-fat", "Cooking fat")
concept("c:butter", "Butter", extends="c:cooking-fat")
concept("c:olive-oil", "Olive oil", extends="c:cooking-fat")
concept("c:starch", "Starch thickener")
concept("c:flour", "Flour", extends="c:starch")
concept("c:cooking-liquid", "Cooking liquid")
concept("c:milk", "Milk", extends="c:cooking-liquid")
concept("c:plant-milk", "Plant milk", extends="c:cooking-liquid")
concept("c:salt", "Salt")
concept("c:nutmeg", "Nutmeg")
concept("c:onion", "Onion")
concept("c:pasta", "Pasta")
concept("c:melting-cheese", "Melting cheese")
concept("c:greens", "Salad greens")
concept("c:vinegar", "Vinegar")
concept("c:bechamel", "Béchamel", note="Milk thickened by a roux, cooked to nappe.")
concept("c:mac-and-cheese", "Mac and cheese")
concept("c:salad", "Green salad")

# ---------- items: varieties, specs, products ----------
std = "Federal standard of identity: at least 80% milkfat"
item("i:butter-unsalted", "Butter, unsalted", ["c:butter"], {"fat": "butter", "salted": False, "milkfat_pct": 80}, source=std)
item("i:butter-salted", "Butter, salted", ["c:butter"], {"fat": "butter", "salted": True, "milkfat_pct": 80}, source=std)
item("i:kerrygold-unsalted", "Kerrygold Unsalted Butter", ["c:butter"], {"fat": "butter", "salted": False, "milkfat_pct": 82}, brand="Kerrygold")
item("i:olive-oil-ev", "Extra-virgin olive oil", ["c:olive-oil"], {"fat": "olive_oil"})
item("i:flour-ap", "All-purpose flour", ["c:flour"], {"thickener": "starch", "gluten": True})
item("i:rice-flour", "Rice flour", ["c:flour"], {"thickener": "starch", "gluten": False})
item("i:milk-whole", "Whole milk", ["c:milk"], {"liquid": "dairy"})
item("i:oat-milk", "Oat milk, unsweetened", ["c:plant-milk"], {"liquid": "plant"})
item("i:salt-kosher", "Kosher salt", ["c:salt"], {})
item("i:nutmeg", "Nutmeg, whole", ["c:nutmeg"], {})
item("i:onion-yellow", "Yellow onion", ["c:onion"], {})
item("i:elbows", "Elbow macaroni", ["c:pasta"], {})
item("i:cheddar-block", "Sharp cheddar, block", ["c:melting-cheese"], {"anti_caking": False, "self_emulsifying": False})
item("i:cheddar-shredded", "Cheddar, pre-shredded bag", ["c:melting-cheese"], {"anti_caking": True, "self_emulsifying": False})
item("i:velveeta", "Velveeta, 8 oz block", ["c:melting-cheese"], {"anti_caking": False, "self_emulsifying": True}, brand="Velveeta")
item("i:mixed-greens", "Mixed greens", ["c:greens"], {})
item("i:red-wine-vinegar", "Red wine vinegar", ["c:vinegar"], {})

# ---------- preparations ----------
bech_facts = {"liquid": "@slot:liquid.liquid", "thickener": "@slot:starch.thickener", "fat": "@slot:fat.fat", "ratio_tbsp_per_cup": 2}
salted_variant = {"when": {"slot": "fat", "fact": "salted", "eq": True},
                  "delta": {"drop_slots": ["salt"], "note": "Salted butter bound: skip the added salt and taste first."}}

def bechamel(fat_bind="i:butter-unsalted", ratio=2, tbsp="4 Tbsp", salt="½ tsp", simmer=6, extra_slots=(), extra_steps_front=(), simmer_text="Lazy bubbles. Stir the corners.", name="Classic Béchamel"):
    facts = dict(bech_facts); facts["ratio_tbsp_per_cup"] = ratio
    return {"kind": "preparation", "name": name, "implements": ["c:bechamel"], "yields": "2 cups",
            "facts": facts,
            "slots": [slot("fat", "fat for roux", "c:cooking-fat", tbsp, fat_bind),
                      slot("starch", "starch", "c:flour", tbsp, "i:flour-ap"),
                      slot("liquid", "liquid", "c:cooking-liquid", "2 cups", "i:milk-whole"),
                      slot("salt", "salt", "c:salt", salt, "i:salt-kosher"),
                      slot("nutmeg", "nutmeg", "c:nutmeg", "pinch", "i:nutmeg"), *extra_slots],
            "steps": [*extra_steps_front,
                      step("WARM", "Milk to steaming.", 3, ["liquid"], cue="wisps of steam, no bubbles"),
                      step("MELT", "Fat into the saucepan, medium-low.", 1, ["fat"]),
                      step("WHISK", "Flour in all at once. Stir constantly.", 2, ["starch"], attend=True, cue="toasty, still pale"),
                      step("WHISK", "Milk in three pours, smooth each time.", 2, ["liquid"], attend=True, cue="no lumps"),
                      step("SIMMER", simmer_text, simmer, [], cue="nappe: a finger line on the spoon holds"),
                      step("SALT", "Salt, then taste.", 0.5, ["salt"]),
                      step("SEASON", "Grate in nutmeg.", 0.5, ["nutmeg"])],
            "variants": [salted_variant]}

commit("p:bechamel-classic", "curated-core", bechamel(), "Classic béchamel, medium")
# Two cooks edit the same parent: competing heads inside one node.
commit("p:bechamel-classic", "chef-luc", bechamel(salt="¾ tsp", simmer=8), "More salt, longer simmer", parent="^")
commit("p:bechamel-classic", "chef-marie", bechamel(simmer=12, simmer_text="Simmer, stirring, then strain."), "Long simmer and strain", parent="@first")

commit("p:bechamel-nana", "nana",
       bechamel(ratio=1.5, tbsp="3 Tbsp", name="Nana's Béchamel",
                extra_slots=[slot("onion", "onion, studded", "c:onion", "¼", "i:onion-yellow")],
                extra_steps_front=[step("INFUSE", "Milk with the studded onion, off heat.", 10, ["liquid", "onion"])]),
       "Lighter, onion-infused", forked_from="p:bechamel-classic")
commit("p:bechamel-olive-oil", "ana", bechamel(fat_bind="i:olive-oil-ev", name="Olive-Oil Béchamel"), "Dairy-light roux")

commit("p:mac-and-cheese-nana", "nana", {
    "kind": "preparation", "name": "Nana's Mac & Cheese", "implements": ["c:mac-and-cheese"], "yields": "4–6",
    "facts": {},
    "slots": [slot("sauce", "cheese sauce base", "c:bechamel", "2 cups"),
              slot("pasta", "pasta", "c:pasta", "8 oz", "i:elbows"),
              slot("cheese", "melting cheese", "c:melting-cheese", "8 oz", "i:cheddar-block")],
    "steps": [step("BOIL", "Elbows into salted water.", 9, ["pasta"], cue="box time − 2"),
              step("MELT", "Off heat: cheese into the sauce by handfuls.", 2, ["sauce", "cheese"], cue="smooth, no oil slick"),
              step("FOLD", "Drained pasta into the sauce. Salt to taste.", 1, ["pasta"])],
    "variants": [
        {"when": {"slot": "cheese", "fact": "self_emulsifying", "eq": True},
         "delta": {"drop_slots": ["sauce"], "at": 1,
                   "add_steps": [step("MELT", "Cubes into 1 cup milk, low heat, stirring.", 6, ["cheese"], attend=True, cue="smooth")],
                   "note": "Processed cheese melts smooth on its own: the béchamel drops out. Omit salt."}},
        {"when": {"slot": "cheese", "fact": "anti_caking", "eq": True},
         "delta": {"note": "Pre-shredded: anti-caking starch can turn the sauce grainy. Lowest heat, small handfuls."}}]})

commit("p:green-salad", "curated-core", {
    "kind": "preparation", "name": "Green Salad", "implements": ["c:salad"], "yields": "4",
    "slots": [slot("greens", "greens", "c:greens", "5 oz", "i:mixed-greens"),
              slot("oil", "oil", "c:olive-oil", "3 Tbsp", "i:olive-oil-ev"),
              slot("vinegar", "vinegar", "c:vinegar", "1 Tbsp", "i:red-wine-vinegar")],
    "steps": [step("WHISK", "Vinegar, salt, then oil in a stream.", 3, ["oil", "vinegar"]),
              step("TOSS", "Dress the greens just before serving.", 1, ["greens"])]})

commit("g:sunday-dinner", "patrick", {
    "kind": "gathering", "name": "Sunday Dinner", "serve": "6:30 PM",
    "slots": [slot("main", "main", "c:mac-and-cheese", "1 pan", "p:mac-and-cheese-nana"),
              slot("side", "side", "c:salad", "1 bowl", "p:green-salad")]})

# ---------- canons ----------
def rule(fact, op, value, label): return {"fact": fact, "op": op, "value": value, "label": label}
ev.append({"type": "canon", "author": "curated-core", "canon": {
    "id": "public-standards", "name": "Public standards", "basis": "Federal standards of identity, paraphrased",
    "contracts": {"c:butter": {"must": [rule("milkfat_pct", "gte", 80, "at least 80% milkfat")]}}}})
ev.append({"type": "canon", "author": "curated-core", "canon": {
    "id": "classical", "name": "Classical French", "basis": "After classical French technique; contracts in our own words",
    "contracts": {"c:bechamel": {"must": [rule("liquid", "eq", "dairy", "dairy liquid"), rule("thickener", "eq", "starch", "starch-thickened")],
                                 "should": [rule("fat", "eq", "butter", "butter roux"), rule("ratio_tbsp_per_cup", "between", [1.5, 3], "1½–3 Tbsp roux per cup")]}},
    "prefer": {"c:bechamel": {"node": "p:bechamel-classic"}}}})
ev.append({"type": "canon", "author": "curated-core", "canon": {
    "id": "home", "name": "Home cooking", "basis": "Whatever works at home",
    "contracts": {"c:bechamel": {"must": [rule("thickener", "eq", "starch", "starch-thickened")]}},
    "prefer": {"c:bechamel": {"node": "p:bechamel-nana"}}}})
ev.append({"type": "canon", "author": "curated-core", "canon": {
    "id": "none", "name": "None / homemade", "basis": "No contracts"}})

# Two groups disagree on the classic's head: main follows Luc; the classical canon follows Marie.
ev.append({"type": "head", "scope": "canon:classical", "node": "p:bechamel-classic", "rev": "@tip", "by": "canon:classical"})

# ---------- evidence and outcomes ----------
for n in ["p:bechamel-classic", "p:bechamel-nana", "p:bechamel-olive-oil", "p:mac-and-cheese-nana"]:
    ev.append({"type": "publish", "node": n, "rev": "^", "by": "seed"})
ev.append({"type": "publish", "node": "p:bechamel-classic", "rev": "@tip", "by": "seed"})
ev.append({"type": "label", "node": "p:bechamel-nana", "rev": "^", "label": "family recipe", "by": "nana"})
ev.append({"type": "label", "node": "p:bechamel-classic", "rev": "^", "label": "endorsed", "by": "canon:classical"})
for cook, node, r, note in [
    ("sam", "p:bechamel-classic", 4, "Reliable."), ("lee", "p:bechamel-classic", 4, ""), ("kai", "p:bechamel-classic", 3, "A bit floury."),
    ("sam", "p:bechamel-nana", 5, "The onion makes it."), ("lee", "p:bechamel-nana", 5, ""), ("rio", "p:bechamel-nana", 4, ""),
    ("kai", "p:bechamel-olive-oil", 3, "Fine for dairy-light."), ("rio", "p:bechamel-olive-oil", 2, "Bitter edge."),
    ("sam", "p:mac-and-cheese-nana", 5, "")]:
    ev.append({"type": "cook_log", "cook": cook, "node": node, "rev": "^", "rating": r, "note": note})

# ---------- the demo cook ----------
ev.append({"type": "profile", "by": "patrick", "profile": {"cook": "patrick", "canons": ["home", "public-standards"], "dial": "advise",
                                          "pantry": {}, "known": [], "choices": {}}})

out = pathlib.Path(__file__).with_name("events.json")
out.write_text(json.dumps(ev, ensure_ascii=False, indent=1))
print(f"{len(ev)} events → {out}")
