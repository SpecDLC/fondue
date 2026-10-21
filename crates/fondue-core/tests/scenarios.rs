//! Each test pins down one load-bearing claim from the design boards.
//! Rev 2: tightened after an independent review.

use fondue_core::api::Engine;
use fondue_core::model::*;
use fondue_core::resolve::{diff, lock, portal, RSlot, Resolved, Resolver};
use fondue_core::seed_events;
use fondue_core::state::{content_id, Event, State};
use serde_json::json;

fn state() -> State {
    State::from_events(seed_events()).expect("seed replays")
}

fn profile(canons: &[&str], dial: Dial) -> Profile {
    Profile { cook: "t".into(), canons: canons.iter().map(|s| s.to_string()).collect(), dial, ..Default::default() }
}

fn view(st: &State, p: &Profile, root: &str) -> Resolved {
    Resolver::new(st, p).resolve_root(root).expect("resolves")
}

fn slot<'a>(r: &'a Resolved, id: &str) -> &'a RSlot {
    r.slots.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("no slot {}", id))
}

fn child<'a>(r: &'a Resolved, id: &str) -> &'a Resolved {
    slot(r, id).child.as_deref().unwrap_or_else(|| panic!("slot {} unbound", id))
}

fn attends(r: &Resolved) -> Vec<String> {
    r.attend.iter().map(|a| format!("{}: {}", a.from, a.verb)).collect()
}

/// Commit a preparation from JSON, panicking on refusal.
fn commit(st: &mut State, node: &str, parent: Option<&str>, body: serde_json::Value) -> String {
    let ev: Event = serde_json::from_value(json!({
        "type": "commit", "node": node, "author": "test", "body": body,
        "parent": parent,
    }))
    .unwrap();
    st.apply(ev).unwrap_or_else(|e| panic!("commit {} refused: {}", node, e));
    st.tip(node).unwrap().clone()
}

fn try_commit(st: &mut State, node: &str, parent: Option<&str>, body: serde_json::Value) -> Result<String, String> {
    let ev: Event = serde_json::from_value(json!({
        "type": "commit", "node": node, "author": "test", "body": body, "parent": parent,
    }))
    .unwrap();
    st.apply(ev)
}

fn classic_body(st: &State) -> serde_json::Value {
    serde_json::to_value(&st.revs[st.head("main", "p:bechamel-classic").unwrap()].body).unwrap()
}

// ---------------------------------------------------------------- log & ids

#[test]
fn stored_log_round_trips_with_concrete_ids() {
    let a = state();
    // Aliases are resolved on append; the stored log carries concrete ids only.
    let text = serde_json::to_string(&a.log).unwrap();
    assert!(!text.contains("\"^\"") && !text.contains("@tip") && !text.contains("@first"));
    // Serialize → parse → replay gives the same revisions, heads and lockfile.
    let b = State::from_events(serde_json::from_str(&text).unwrap()).unwrap();
    assert_eq!(a.revs.keys().collect::<Vec<_>>(), b.revs.keys().collect::<Vec<_>>());
    assert_eq!(a.heads, b.heads);
    let p = profile(&["home"], Dial::Advise);
    assert_eq!(lock(&view(&a, &p, "g:sunday-dinner")), lock(&view(&b, &p, "g:sunday-dinner")));
}

#[test]
fn content_ids_are_golden() {
    // If this changes, every stored id changes. Bump state::SCHEMA deliberately, never by accident.
    let body: Body = serde_json::from_value(json!({"kind": "item", "name": "Butter, unsalted", "implements": ["c:butter"], "facts": {"fat": "butter"}})).unwrap();
    let id = content_id("i:butter", &None, "curated-core", "", &body, &None);
    assert_eq!(id, GOLDEN, "content id drifted: {}", id);
    // Adding an empty optional field does not change the id.
    let with_empty: Body = serde_json::from_value(json!({"kind": "item", "name": "Butter, unsalted", "implements": ["c:butter"], "facts": {"fat": "butter"}, "source": null})).unwrap();
    assert_eq!(content_id("i:butter", &None, "curated-core", "", &with_empty, &None), GOLDEN);
}
const GOLDEN: &str = include_str!("golden_id.txt");

#[test]
fn step_why_leaves_old_ids_alone() {
    // A step that does not set `why` has the same canonical JSON, hence the same id, as before the field existed.
    let step = json!({"verb": "WHISK", "text": "Whisk.", "mins": 1.0, "attend": true, "uses": [], "cue": "smooth"});
    let mk = |step: serde_json::Value| -> String {
        let body: Body = serde_json::from_value(json!({"kind": "preparation", "name": "T", "slots": [], "steps": [step]})).unwrap();
        content_id("p:t", &None, "t", "", &body, &None)
    };
    let plain = mk(step.clone());
    let mut empty = step.clone();
    empty["why"] = json!("");
    assert_eq!(mk(empty), plain, "an empty why must not change the id");
    // Pinned: the id of a step with no why, taken before the field was added.
    assert_eq!(plain, STEP_PLAIN_ID, "id of a step without why drifted: {}", plain);
    // A step that sets why is a different revision.
    let mut with = step;
    with["why"] = json!("So it does not split.");
    assert_ne!(mk(with.clone()), plain);
    // And the text survives a round trip.
    let b: Body = serde_json::from_value(json!({"kind": "preparation", "name": "T", "slots": [], "steps": [with]})).unwrap();
    let back: serde_json::Value = serde_json::to_value(&b).unwrap();
    assert_eq!(back["steps"][0]["why"], "So it does not split.");
}
const STEP_PLAIN_ID: &str = "12dc9f0dd558c65a4d71218cf1fcd481";

#[test]
fn malformed_events_are_refused() {
    // A typo is an error, not a silently different event.
    let bad = serde_json::from_value::<Event>(json!({"type": "commit", "node": "p:x", "parnet": "^", "author": "a", "body": {"kind": "concept", "name": "x"}}));
    assert!(bad.is_err());
    let bad_slot = serde_json::from_value::<Body>(json!({"kind": "preparation", "name": "x", "slots": [{"id": "a", "role": "r", "concept": "c:x", "qyt": "1"}]}));
    assert!(bad_slot.is_err());
}

#[test]
fn duplicate_commits_are_refused_not_dropped() {
    let mut st = state();
    let body = json!({"kind": "concept", "name": "Dup"});
    commit(&mut st, "c:dup", None, body.clone());
    // Same node again without a parent is refused (it already exists).
    assert!(try_commit(&mut st, "c:dup", None, body.clone()).is_err());
    // Identical commit on the same parent is refused, not silently ignored.
    let first = st.tip("c:dup").unwrap().clone();
    commit(&mut st, "c:dup", Some(&first), json!({"kind": "concept", "name": "Dup 2"}));
    assert!(try_commit(&mut st, "c:dup", Some(&first), json!({"kind": "concept", "name": "Dup 2"})).unwrap_err().contains("already exists"));
}

#[test]
fn rejected_events_leave_no_trace() {
    let mut e = Engine::default();
    e.state = state();
    let n = e.state.log.len();
    let out = e.call(r#"{"op":"append","event":{"type":"cook_log","cook":"x","node":"p:bechamel-nana","rev":"^","rating":9}}"#);
    assert!(out.contains("\"ok\":false"));
    assert_eq!(e.state.log.len(), n);
}

// ---------------------------------------------------------------- levels & binding

#[test]
fn levels_are_derived_from_nesting() {
    let st = state();
    let p = profile(&["home"], Dial::Advise);
    let dinner = view(&st, &p, "g:sunday-dinner");
    assert_eq!(dinner.level, "meal");
    let mac = child(&dinner, "main");
    assert_eq!(mac.level, "dish");
    let bech = child(mac, "sauce");
    assert_eq!(bech.level, "component");
    assert_eq!(child(bech, "fat").level, "ingredient");
    assert_eq!(view(&st, &p, "p:bechamel-nana").level, "dish");
}

#[test]
fn canon_suggests_but_cook_is_asked_once() {
    let st = state();
    let p = profile(&["home"], Dial::Advise);
    let mac = view(&st, &p, "p:mac-and-cheese-nana");
    assert_eq!(slot(&mac, "sauce").via, "canon:home");
    assert!(slot(&mac, "sauce").needs_choice);
    let mut p2 = p.clone();
    p2.choices.insert("p:mac-and-cheese-nana/sauce".into(), "p:bechamel-olive-oil".into());
    let mac2 = view(&st, &p2, "p:mac-and-cheese-nana");
    assert_eq!(slot(&mac2, "sauce").via, "choice");
    assert!(!slot(&mac2, "sauce").needs_choice);
    let none = view(&st, &profile(&["none"], Dial::Advise), "p:mac-and-cheese-nana");
    assert!(slot(&none, "sauce").child.is_none() && slot(&none, "sauce").needs_choice);
}

#[test]
fn salted_butter_in_pantry_drops_added_salt() {
    let st = state();
    let mut p = profile(&["classical"], Dial::Advise);
    assert!(view(&st, &p, "p:bechamel-classic").steps.iter().any(|s| s.verb == "SALT"));
    // "I keep salted butter" — keyed by butter, applied to a slot asking for cooking fat.
    p.pantry.insert("c:butter".into(), "i:butter-salted".into());
    let salted = view(&st, &p, "p:bechamel-classic");
    let fat = slot(&salted, "fat");
    assert_eq!(fat.via, "pantry");
    assert!(fat.substituted);
    assert!(slot(&salted, "salt").dropped);
    assert!(!salted.steps.iter().any(|s| s.verb == "SALT"));
}

#[test]
fn pantry_does_not_override_a_different_kind_of_default() {
    let st = state();
    let mut p = profile(&["home"], Dial::Advise);
    p.pantry.insert("c:butter".into(), "i:butter-salted".into());
    // The olive-oil béchamel's author chose olive oil on purpose; keeping butter doesn't change that.
    let oo = view(&st, &p, "p:bechamel-olive-oil");
    assert_eq!(child(&oo, "fat").node, "i:olive-oil-ev");
    assert_eq!(slot(&oo, "fat").via, "recipe");
}

#[test]
fn velveeta_removes_the_bechamel_and_its_attention() {
    let st = state();
    let mut p = profile(&["home"], Dial::Advise);
    p.choices.insert("p:mac-and-cheese-nana/sauce".into(), "p:bechamel-classic".into());
    let before = view(&st, &p, "p:mac-and-cheese-nana");
    assert!(before.attend.iter().any(|a| a.from.contains("Béchamel")));
    p.choices.insert("p:mac-and-cheese-nana/cheese".into(), "i:velveeta".into());
    let after = view(&st, &p, "p:mac-and-cheese-nana");
    assert!(slot(&after, "sauce").dropped);
    assert!(!after.attend.iter().any(|a| a.from.contains("Béchamel")));
    assert!(after.total_mins < before.total_mins);
    assert!(!lock(&after).iter().any(|l| l.node.contains("bechamel")));
}

#[test]
fn delta_inserts_are_positioned_against_the_post_drop_list() {
    let mut st = state();
    let s = |v: &str| json!({"verb": v, "text": v, "mins": 1});
    commit(&mut st, "p:order", None, json!({
        "kind": "preparation", "name": "Order", "implements": [],
        "slots": [{"id": "x", "role": "x", "concept": "c:butter", "qty": "1", "bind": {"node": "i:butter-salted"}}],
        "steps": [s("S0"), s("S1"), s("S2")],
        "variants": [
            {"when": {"slot": "x", "fact": "salted", "eq": true}, "delta": {"at": 1, "add_steps": [s("A")], "note": "a"}},
            {"when": {"slot": "x", "fact": "salted", "eq": true}, "delta": {"at": 2, "add_steps": [s("B")], "note": "b"}}
        ]
    }));
    let r = view(&st, &profile(&["home"], Dial::Off), "p:order");
    let verbs: Vec<&str> = r.steps.iter().map(|s| s.verb.as_str()).collect();
    assert_eq!(verbs, ["S0", "A", "S1", "B", "S2"]);
}

#[test]
fn a_variant_does_not_fire_on_a_slot_another_variant_dropped() {
    let mut st = state();
    commit(&mut st, "p:chain", None, json!({
        "kind": "preparation", "name": "Chain", "implements": [],
        "slots": [
            {"id": "x", "role": "x", "concept": "c:butter", "qty": "1", "bind": {"node": "i:butter-salted"}},
            {"id": "y", "role": "y", "concept": "c:butter", "qty": "1", "bind": {"node": "i:butter-salted"}}
        ],
        "steps": [{"verb": "S", "text": "s", "mins": 1}],
        "variants": [
            {"when": {"slot": "x", "fact": "salted", "eq": true}, "delta": {"drop_slots": ["y"], "note": "drop y"}},
            {"when": {"slot": "y", "fact": "salted", "eq": true}, "delta": {"add_steps": [{"verb": "GHOST", "text": "g", "mins": 1}], "note": "from y"}}
        ]
    }));
    let r = view(&st, &profile(&["home"], Dial::Off), "p:chain");
    assert!(!r.steps.iter().any(|s| s.verb == "GHOST"));
    assert_eq!(r.deltas, vec!["drop y".to_string()]);
}

// ---------------------------------------------------------------- contracts & dial

#[test]
fn enforce_falls_through_to_a_different_candidate() {
    let mut st = state();
    // An oat-milk béchamel the cook picked; classical requires dairy.
    let mut b = classic_body(&st);
    b["name"] = json!("Oat Béchamel");
    for s in b["slots"].as_array_mut().unwrap() {
        if s["id"] == "liquid" {
            s["bind"] = json!({"node": "i:oat-milk"});
        }
    }
    commit(&mut st, "p:bechamel-oat", None, b);
    let mut p = profile(&["classical"], Dial::Enforce);
    p.choices.insert("p:mac-and-cheese-nana/sauce".into(), "p:bechamel-oat".into());
    let mac = view(&st, &p, "p:mac-and-cheese-nana");
    let s = slot(&mac, "sauce");
    assert_eq!(s.blocked.len(), 1, "{:?}", s.blocked);
    assert!(s.blocked[0].contains("Oat Béchamel") && s.blocked[0].contains("dairy liquid"));
    // Fell through to the canon's preference: a different node, actually bound.
    assert_eq!(s.via, "canon:classical");
    assert_eq!(child(&mac, "sauce").node, "p:bechamel-classic");

    // Advise binds the cook's choice and flags it.
    p.dial = Dial::Advise;
    let adv = view(&st, &p, "p:mac-and-cheese-nana");
    assert_eq!(child(&adv, "sauce").node, "p:bechamel-oat");
    assert_eq!(slot(&adv, "sauce").check.status, "violates");
    // Off binds it without checking.
    p.dial = Dial::Off;
    assert_eq!(slot(&view(&st, &p, "p:mac-and-cheese-nana"), "sauce").check.status, "unchecked");
}

#[test]
fn butter_contract_applies_through_a_cooking_fat_slot() {
    let mut st = state();
    st.apply(serde_json::from_value(json!({"type": "commit", "node": "i:butter-thin", "author": "test",
        "body": {"kind": "item", "name": "Butter-ish spread", "implements": ["c:butter"], "facts": {"fat": "butter", "salted": false, "milkfat_pct": 50}}})).unwrap()).unwrap();
    let mut p = profile(&["public-standards", "classical"], Dial::Enforce);
    p.choices.insert("p:bechamel-classic/fat".into(), "i:butter-thin".into());
    let r = view(&st, &p, "p:bechamel-classic");
    let fat = slot(&r, "fat");
    // The roux slot asks for c:cooking-fat; the spread still answers to butter's rule.
    assert!(fat.blocked.iter().any(|b| b.contains("80% milkfat")), "{:?}", fat.blocked);
    assert_eq!(child(&r, "fat").node, "i:butter-unsalted");
    assert_eq!(fat.check.status, "ok");
}

#[test]
fn olive_oil_bechamel_deviates_under_classical_but_is_ok_at_home() {
    let st = state();
    let classical = portal(&st, &profile(&["classical"], Dial::Advise), "c:bechamel");
    let oo = classical.iter().find(|e| e.node == "p:bechamel-olive-oil").unwrap();
    assert_eq!(oo.check.status, "deviates");
    assert_eq!(oo.check.should_fail, vec!["butter roux".to_string()]);
    let home = portal(&st, &profile(&["home"], Dial::Advise), "c:bechamel");
    assert_eq!(home.iter().find(|e| e.node == "p:bechamel-olive-oil").unwrap().check.status, "ok");
    assert_eq!(classical[0].node, "p:bechamel-classic");
}

// ---------------------------------------------------------------- heads & cycles

#[test]
fn competing_heads_resolve_per_canon() {
    let st = state();
    assert_eq!(st.tips("p:bechamel-classic").len(), 2);
    let main_rev = st.head("main", "p:bechamel-classic").unwrap().clone();
    let classical_rev = st.head("canon:classical", "p:bechamel-classic").unwrap().clone();
    assert_eq!(st.revs[&main_rev].author, "chef-luc");
    assert_eq!(st.revs[&classical_rev].author, "chef-marie");
    // Both competing revisions are published, explicitly.
    assert!(st.published.contains(&main_rev) && st.published.contains(&classical_rev));
    let a = view(&st, &profile(&["classical"], Dial::Advise), "p:bechamel-classic");
    let b = view(&st, &profile(&["home"], Dial::Advise), "p:bechamel-classic");
    assert_eq!(a.rev, classical_rev);
    assert_eq!(b.rev, main_rev);
    let (la, lb) = (lock(&a), lock(&b));
    assert_eq!(la.iter().zip(lb.iter()).filter(|(x, y)| x != y).count(), 1);
    assert!(diff(&st, &main_rev, &classical_rev).unwrap().iter().any(|l| l.contains("strain")));
}

#[test]
fn cycles_are_refused_at_commit_including_pinned_revisions() {
    let mut st = state();
    let a1 = commit(&mut st, "p:loop-a", None, json!({"kind": "preparation", "name": "A", "implements": ["c:bechamel"]}));
    // B pins A's specific revision.
    commit(&mut st, "p:loop-b", None, json!({"kind": "preparation", "name": "B", "implements": ["c:bechamel"],
        "slots": [{"id": "a", "role": "a", "concept": "c:bechamel", "bind": {"node": "p:loop-a", "rev": a1}}]}));
    // A new revision of A that pins B closes the loop through the pinned revision.
    let err = try_commit(&mut st, "p:loop-a", Some("^"), json!({"kind": "preparation", "name": "A2", "implements": ["c:bechamel"],
        "slots": [{"id": "b", "role": "b", "concept": "c:bechamel", "bind": {"node": "p:loop-b"}}]})).unwrap_err();
    assert!(err.contains("cycle"), "{}", err);
}

#[test]
fn cycles_are_refused_when_a_head_moves() {
    let mut st = state();
    let x1 = commit(&mut st, "p:x", None, json!({"kind": "preparation", "name": "X1", "implements": ["c:bechamel"]}));
    // A side revision of X that pins mac and cheese. No one points at it yet.
    let xr = commit(&mut st, "p:x", Some(&x1), json!({"kind": "preparation", "name": "X-side", "implements": ["c:bechamel"],
        "slots": [{"id": "m", "role": "m", "concept": "c:mac-and-cheese", "bind": {"node": "p:mac-and-cheese-nana"}}]}));
    // Main moves on past it, so the side revision is neither a head nor the tip.
    st.apply(serde_json::from_value(json!({"type": "head", "scope": "main", "node": "p:x", "rev": x1})).unwrap()).unwrap();
    commit(&mut st, "p:x", Some(&x1), json!({"kind": "preparation", "name": "X3", "implements": ["c:bechamel"]}));
    // Mac and cheese pins X (no revision): fine for now.
    let mac = serde_json::to_value(&st.revs[st.head("main", "p:mac-and-cheese-nana").unwrap()].body).unwrap();
    let mut mac2 = mac.clone();
    mac2["slots"][0]["bind"] = json!({"node": "p:x"});
    commit(&mut st, "p:mac-and-cheese-nana", Some("^"), mac2);
    // A canon pointing its head at the side revision would close the loop.
    let err = st.apply(serde_json::from_value(json!({"type": "head", "scope": "canon:weird", "node": "p:x", "rev": xr})).unwrap()).unwrap_err();
    assert!(err.contains("cycle"), "{}", err);
}

#[test]
fn resolve_time_cycles_are_caught_too() {
    let mut st = state();
    commit(&mut st, "p:bechamel-weird", None, json!({"kind": "preparation", "name": "Weird", "implements": ["c:bechamel"],
        "slots": [{"id": "x", "role": "x", "concept": "c:mac-and-cheese"}]}));
    let mut p = profile(&["home"], Dial::Off);
    p.choices.insert("p:mac-and-cheese-nana/sauce".into(), "p:bechamel-weird".into());
    p.choices.insert("p:bechamel-weird/x".into(), "p:mac-and-cheese-nana".into());
    let err = Resolver::new(&st, &p).resolve_root("p:mac-and-cheese-nana").unwrap_err();
    assert!(err.contains("cycle"), "{}", err);
}

#[test]
fn bad_pins_are_refused() {
    let mut st = state();
    // A pin whose revision belongs to another node is refused at commit.
    let other = st.tip("i:butter-salted").unwrap().clone();
    let err = try_commit(&mut st, "p:badpin", None, json!({"kind": "preparation", "name": "Bad",
        "slots": [{"id": "f", "role": "f", "concept": "c:butter", "bind": {"node": "i:butter-unsalted", "rev": other}}]})).unwrap_err();
    assert!(err.contains("belongs to"), "{}", err);
    // A pin to a node that doesn't exist is refused at commit.
    assert!(try_commit(&mut st, "p:badpin", None, json!({"kind": "preparation", "name": "Bad",
        "slots": [{"id": "f", "role": "f", "concept": "c:butter", "bind": {"node": "i:nope"}}]})).is_err());
}

// ---------------------------------------------------------------- folding & outcomes

#[test]
fn known_components_fold_but_attention_bubbles() {
    let st = state();
    let mut p = profile(&["home"], Dial::Advise);
    p.choices.insert("p:mac-and-cheese-nana/sauce".into(), "p:bechamel-nana".into());
    p.known.insert("c:bechamel".into());
    let mac = view(&st, &p, "p:mac-and-cheese-nana");
    assert!(child(&mac, "sauce").folded);
    assert!(attends(&mac).iter().any(|a| a.starts_with("Nana's Béchamel: ")));
}

#[test]
fn outcomes_are_per_revision() {
    let mut st = state();
    let luc = st.head("main", "p:bechamel-classic").unwrap().clone();
    let marie = st.head("canon:classical", "p:bechamel-classic").unwrap().clone();
    let before = st.outcome_rev(&marie).0;
    st.apply(serde_json::from_value(json!({"type": "cook_log", "cook": "z", "node": "p:bechamel-classic", "rev": marie, "rating": 5})).unwrap()).unwrap();
    assert_eq!(st.outcome_rev(&marie).0, before + 1);
    assert_eq!(st.outcome_rev(&luc).0, 3, "the seed's three ratings went to main (Luc)");
    assert_eq!(st.outcome_node("p:bechamel-classic").0, 4);
}

#[test]
fn outcomes_fork_publish_loop() {
    let mut e = Engine::default();
    e.state = state();
    let tip = e.state.head("main", "p:bechamel-nana").unwrap().clone();
    let mut body = e.state.revs[&tip].body.clone();
    if let Body::Preparation(pb) = &mut body {
        pb.name = "Patrick's Béchamel".into();
        pb.facts.insert("ratio_tbsp_per_cup".into(), json!(2.5));
        pb.slots.iter_mut().find(|s| s.id == "fat").unwrap().bind = Some(Pin { node: "i:kerrygold-unsalted".into(), rev: None });
    }
    let commit = Event::Commit { node: "p:bechamel-patrick".into(), parent: None, author: "patrick".into(), msg: "Dialed in".into(), body, forked_from: Some(Pin { node: "p:bechamel-nana".into(), rev: Some(tip.clone()) }) };
    assert!(e.call(&json!({ "op": "append", "event": commit }).to_string()).contains("\"ok\":true"));
    for ev in [
        json!({"type":"publish","node":"p:bechamel-patrick","rev":"^","by":"patrick"}),
        json!({"type":"cook_log","cook":"patrick","node":"p:bechamel-patrick","rev":"^","rating":5,"note":"Perfect"}),
        json!({"type":"cook_log","cook":"sam","node":"p:bechamel-patrick","rev":"^","rating":5,"note":""}),
    ] {
        let out = e.call(&json!({ "op": "append", "event": ev }).to_string());
        assert!(out.contains("\"ok\":true"), "{}", out);
    }
    let pt = portal(&e.state, &profile(&["none"], Dial::Advise), "c:bechamel");
    assert_eq!(pt[0].node, "p:bechamel-patrick");
    assert_eq!(pt[0].forked_from.as_ref().unwrap().node, "p:bechamel-nana");
    let d = diff(&e.state, &tip, &e.state.head("main", "p:bechamel-patrick").unwrap().clone()).unwrap();
    assert!(d.iter().any(|l| l.contains("kerrygold")), "{:?}", d);
}

#[test]
fn preference_events_edit_one_field() {
    let mut st = state();
    let before = st.profiles["patrick"].clone();
    for set in [json!({"op": "dial", "dial": "enforce"}), json!({"op": "know", "concept": "c:bechamel", "on": true}),
                json!({"op": "pantry", "concept": "c:butter", "node": "i:butter-salted"})] {
        st.apply(serde_json::from_value(json!({"type": "pref", "cook": "patrick", "set": set})).unwrap()).unwrap();
    }
    let p = &st.profiles["patrick"];
    assert_eq!(p.dial, Dial::Enforce);
    assert!(p.known.contains("c:bechamel"));
    assert_eq!(p.pantry["c:butter"], "i:butter-salted");
    assert_eq!(p.canons, before.canons, "untouched fields survive");
}
