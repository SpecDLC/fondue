//! A visitor's saved log moves onto a newer seed: their own pref, cook_log and
//! session events are replayed on top of it, and the seed's own content is new.

use fondue_core::api::Engine;
use fondue_core::seed_events;
use serde_json::{json, Value};

fn run(e: &mut Engine, cmd: Value) -> Value {
    serde_json::from_str(&e.call(&cmd.to_string())).unwrap()
}
fn ok(e: &mut Engine, cmd: Value) -> Value {
    let r = run(e, cmd);
    assert_eq!(r["ok"], true, "{}", r);
    r["data"].clone()
}
fn seed() -> Vec<Value> {
    seed_events().into_iter().map(|e| serde_json::to_value(e).unwrap()).collect()
}
/// A newer seed: the same events plus one content change (a new recipe).
fn newer_seed() -> Vec<Value> {
    let mut e = Engine::default();
    ok(&mut e, json!({"op": "load", "events": seed()}));
    let recipe = json!({"id": "new-in-this-release", "title": "New In This Release", "serves": "2",
        "slots": [{ "id": "fat", "role": "fat", "concept": "c:cooking-fat", "qty": "1 Tbsp", "bind": "i:butter-unsalted" }],
        "steps": [{ "verb": "MELT", "text": "Melt the fat.", "mins": 1, "attend": true, "uses": ["fat"], "cue": "foaming" }]});
    ok(&mut e, json!({"op": "import", "recipe": recipe, "cook": "fondue"}));
    ok(&mut e, json!({"op": "events"})).as_array().unwrap().clone()
}
/// What a returning visitor has saved: the old seed and their own edits.
fn visitor_log(e: &mut Engine) -> Vec<Value> {
    ok(e, json!({"op": "load", "events": seed()}));
    let rated = ok(e, json!({"op": "append", "event": {"type": "cook_log", "cook": "patrick", "node": "p:bechamel-nana", "rev": "^", "rating": 4}}))["seq"].clone();
    ok(e, json!({"op": "append", "event": {"type": "cook_log", "cook": "patrick", "node": "p:bechamel-nana", "rev": "^", "rating": 1}}));
    let taken_back = rated.as_u64().unwrap() + 1;
    ok(e, json!({"op": "append", "event": {"type": "retract", "seq": taken_back, "by": "patrick"}}));
    ok(e, json!({"op": "append", "event": {"type": "pref", "cook": "patrick", "set": {"op": "pantry", "concept": "c:butter", "node": "i:butter-salted"}}}));
    ok(e, json!({"op": "append", "event": {"type": "session", "cook": "patrick", "session": {"recipe": "p:bechamel-nana", "active": true, "step": 1}}}));
    ok(e, json!({"op": "events"})).as_array().unwrap().clone()
}

#[test]
fn the_seed_has_an_id_that_changes_with_its_content() {
    let mut e = Engine::default();
    let a = ok(&mut e, json!({"op": "seed_id", "events": seed()}));
    let b = ok(&mut e, json!({"op": "seed_id", "events": newer_seed()}));
    assert!(a.as_str().unwrap().len() >= 16);
    assert_ne!(a, b);
    assert_eq!(a, ok(&mut e, json!({"op": "seed_id", "events": seed()})));
}

#[test]
fn a_returning_visitor_gets_the_new_seed_and_keeps_their_own_events() {
    let mut e = Engine::default();
    let saved = visitor_log(&mut e);
    let r = ok(&mut e, json!({"op": "rebase", "seed": newer_seed(), "saved": saved, "saved_seed_len": seed().len()}));
    assert_eq!(r["replayed"], 3, "{}", r);   // one rating (the taken-back one is dropped with its retract), the pantry pref, the session
    let log = ok(&mut e, json!({"op": "events"})).as_array().unwrap().clone();
    let n = newer_seed().len();
    assert_eq!(log.len(), n + 3);
    assert!(ok(&mut e, json!({"op": "catalog"})).as_array().unwrap().iter().any(|c| c["node"] == "p:new-in-this-release"), "the new seed's content is missing");
    assert_eq!(ok(&mut e, json!({"op": "profile", "cook": "patrick"}))["pantry"]["c:butter"], "i:butter-salted");
    assert_eq!(ok(&mut e, json!({"op": "session", "cook": "patrick"}))["step"], 1);
}

#[test]
fn a_saved_log_with_no_seed_id_is_rebased_when_it_starts_like_the_seed() {
    // Logs saved before seeds had ids. A content fix keeps the seed's shape (the same kinds of events in the
    // same order), so the first seed-length events of the saved log are taken to be its seed.
    let mut fixed = seed();
    let last = fixed.len() - 1;
    fixed[last]["profile"]["dial"] = json!("enforce");
    let mut e = Engine::default();
    assert_ne!(ok(&mut e, json!({"op": "seed_id", "events": fixed})), ok(&mut e, json!({"op": "seed_id", "events": seed()})));
    let saved = visitor_log(&mut e);
    let r = ok(&mut e, json!({"op": "rebase", "seed": fixed, "saved": saved}));
    assert_eq!(r["replayed"], 3, "{}", r);
    let p = ok(&mut e, json!({"op": "profile", "cook": "patrick"}));
    assert_eq!(p["dial"], "enforce", "the seed's fix did not reach the visitor");
    assert_eq!(p["pantry"]["c:butter"], "i:butter-salted", "the visitor's own pref was lost");
    // A log that does not start like the seed is refused rather than guessed at.
    let r = run(&mut e, json!({"op": "rebase", "seed": newer_seed(), "saved": [] }));
    assert_eq!(r["ok"], false);
}

#[test]
fn a_visitor_event_that_no_longer_fits_is_skipped_not_fatal() {
    let mut e = Engine::default();
    let mut saved = visitor_log(&mut e);
    saved.push(json!({"type": "session", "cook": "patrick", "session": {"recipe": "p:gone-from-the-seed"}}));
    let r = ok(&mut e, json!({"op": "rebase", "seed": newer_seed(), "saved": saved, "saved_seed_len": seed().len()}));
    assert_eq!(r["replayed"], 3);
    assert_eq!(r["skipped"], 1);
}

#[test]
fn edits_and_undos_survive_a_new_seed_and_undo_points_at_the_moved_edit() {
    let mut e = Engine::default();
    let old_seed = seed();
    ok(&mut e, json!({"op": "load", "events": old_seed}));
    let pantry = |n: &str| json!({"type": "pref", "cook": "patrick", "set": {"op": "pantry", "concept": "c:butter", "node": n}});
    let prev = ok(&mut e, json!({"op": "profile", "cook": "patrick"}))["pantry"]["c:butter"].clone();
    let undo_prev = json!({"type": "pref", "cook": "patrick", "set": {"op": "pantry", "concept": "c:butter", "node": prev}});
    // Edit 1, undone; edit 2 kept with its Undo still available.
    ok(&mut e, json!({"op": "append", "event": pantry("i:butter-salted")}));
    let s1 = ok(&mut e, json!({"op": "append", "event": {"type": "edit", "by": "patrick", "text": "Set butter salted", "undo": [undo_prev]}}))["seq"].as_u64().unwrap();
    ok(&mut e, json!({"op": "append", "event": {"type": "undo", "undoes": s1, "by": "patrick"}}));
    ok(&mut e, json!({"op": "append", "event": pantry("i:butter-salted")}));
    let s2 = ok(&mut e, json!({"op": "append", "event": {"type": "edit", "by": "agent", "text": "Set butter salted again", "undo": [undo_prev]}}))["seq"].as_u64().unwrap();
    let saved = ok(&mut e, json!({"op": "events"}));

    let new_seed = newer_seed();
    let shift = (new_seed.len() - old_seed.len()) as u64;
    let mut fresh = Engine::default();
    let r = ok(&mut fresh, json!({"op": "rebase", "seed": new_seed, "saved": saved, "saved_seed_len": old_seed.len()}));
    assert_eq!(r["skipped"], 0, "{}", r);
    let edits = ok(&mut fresh, json!({"op": "edits"}));
    let edits = edits.as_array().unwrap();
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0]["seq"], s1 + shift);
    assert_eq!(edits[0]["undone"], true);
    assert_eq!(edits[1]["seq"], s2 + shift);
    assert_eq!(edits[1]["undone"], false);
    let log = ok(&mut fresh, json!({"op": "events"}));
    assert!(log.as_array().unwrap().iter().any(|ev| ev["type"] == "undo" && ev["undoes"] == s1 + shift), "{}", log);
    // The kept edit can still be undone once, in the new log.
    ok(&mut fresh, json!({"op": "append", "event": {"type": "undo", "undoes": s2 + shift, "by": "patrick"}}));
    assert_eq!(run(&mut fresh, json!({"op": "append", "event": {"type": "undo", "undoes": s2 + shift, "by": "patrick"}}))["ok"], false);
}
