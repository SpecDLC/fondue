//! One Undo path for every edit (FONDUE-SITE-12): an `edit` record names an edit
//! and the events that put it back; an `undo` event reverses one, is labeled with
//! the edit it undoes, survives a replay, and is refused when the field has changed since.

use fondue_core::api::Engine;
use fondue_core::seed_events;
use serde_json::{json, Value};

fn engine() -> Engine {
    let mut e = Engine::default();
    let r: Value = serde_json::from_str(&e.call(&json!({"op": "load", "events": seed_events()}).to_string())).unwrap();
    assert_eq!(r["ok"], true);
    e
}
fn run(e: &mut Engine, cmd: Value) -> Value {
    serde_json::from_str(&e.call(&cmd.to_string())).unwrap()
}
fn ok(e: &mut Engine, cmd: Value) -> Value {
    let r = run(e, cmd);
    assert_eq!(r["ok"], true, "{}", r);
    r["data"].clone()
}
fn log(e: &mut Engine) -> Vec<Value> {
    ok(e, json!({"op": "events"})).as_array().unwrap().clone()
}
fn butter(e: &mut Engine) -> Value {
    ok(e, json!({"op": "profile", "cook": "patrick"}))["pantry"]["c:butter"].clone()
}
fn pantry(node: Value) -> Value {
    json!({"type": "pref", "cook": "patrick", "set": {"op": "pantry", "concept": "c:butter", "node": node}})
}
/// Set butter and record the edit, the way both the page and window.fondue do. Returns the edit's seq.
fn set_butter(e: &mut Engine, by: &str, node: &str) -> usize {
    let prev = butter(e);
    ok(e, json!({"op": "append", "event": pantry(json!(node))}));
    let r = ok(e, json!({"op": "append", "event": {"type": "edit", "by": by, "text": format!("Set butter to {}", node), "undo": [pantry(prev)]}}));
    r["seq"].as_u64().unwrap() as usize
}

#[test]
fn undo_reverses_a_hand_edit_and_an_agent_edit_and_is_labeled() {
    let mut e = engine();
    let b0 = butter(&mut e);
    let hand = set_butter(&mut e, "patrick", "i:butter-salted");
    let agent = set_butter(&mut e, "agent", "i:butter-unsalted");
    let before = log(&mut e);
    ok(&mut e, json!({"op": "append", "event": {"type": "undo", "undoes": agent, "by": "patrick"}}));
    assert_eq!(butter(&mut e), json!("i:butter-salted"));
    ok(&mut e, json!({"op": "append", "event": {"type": "undo", "undoes": hand, "by": "patrick"}}));
    assert_eq!(butter(&mut e), b0);
    let after = log(&mut e);
    assert_eq!(after.len(), before.len() + 2, "each undo is one appended event");
    assert_eq!(&after[..before.len()], &before[..], "undo never rewrites an earlier event");
    assert_eq!(after[before.len()]["type"], "undo");
    assert_eq!(after[before.len()]["undoes"], agent);
    let edits = ok(&mut e, json!({"op": "edits"}));
    let edits = edits.as_array().unwrap();
    assert_eq!(edits.len(), 2);
    assert!(edits.iter().all(|x| x["undone"] == true), "{:?}", edits);
}

#[test]
fn undo_survives_a_reload_of_the_log() {
    let mut e = engine();
    let first = set_butter(&mut e, "patrick", "i:butter-salted");
    let second = set_butter(&mut e, "agent", "i:butter-unsalted");
    ok(&mut e, json!({"op": "append", "event": {"type": "undo", "undoes": second, "by": "patrick"}}));
    let saved = log(&mut e);
    let mut fresh = Engine::default();
    ok(&mut fresh, json!({"op": "load", "events": saved}));
    assert_eq!(butter(&mut fresh), json!("i:butter-salted"));
    let edits = ok(&mut fresh, json!({"op": "edits"}));
    let by_seq = |s: usize| edits.as_array().unwrap().iter().find(|x| x["seq"] == s).cloned().unwrap();
    assert_eq!(by_seq(second)["undone"], true);
    assert_eq!(by_seq(first)["undone"], false);
    assert_eq!(by_seq(first)["stale"], false);
    ok(&mut fresh, json!({"op": "append", "event": {"type": "undo", "undoes": first, "by": "patrick"}}));
    assert_eq!(butter(&mut fresh), butter(&mut engine()));
}

#[test]
fn a_stale_undo_is_refused_with_a_plain_message_and_changes_nothing() {
    let mut e = engine();
    let agent = set_butter(&mut e, "agent", "i:butter-unsalted");
    set_butter(&mut e, "patrick", "i:butter-salted");
    let n = log(&mut e).len();
    let edits = ok(&mut e, json!({"op": "edits"}));
    assert_eq!(edits.as_array().unwrap().iter().find(|x| x["seq"] == agent).unwrap()["stale"], true);
    let r = run(&mut e, json!({"op": "append", "event": {"type": "undo", "undoes": agent, "by": "patrick"}}));
    assert_eq!(r["ok"], false);
    assert!(r["error"].as_str().unwrap().contains("changed since"), "{}", r);
    assert_eq!(butter(&mut e), json!("i:butter-salted"));
    assert_eq!(log(&mut e).len(), n);
}

#[test]
fn an_edit_is_undone_once_and_only_an_edit_can_be_undone() {
    let mut e = engine();
    let s = set_butter(&mut e, "patrick", "i:butter-salted");
    ok(&mut e, json!({"op": "append", "event": {"type": "undo", "undoes": s, "by": "patrick"}}));
    let twice = run(&mut e, json!({"op": "append", "event": {"type": "undo", "undoes": s, "by": "patrick"}}));
    assert_eq!(twice["ok"], false);
    let not_edit = run(&mut e, json!({"op": "append", "event": {"type": "undo", "undoes": 0, "by": "patrick"}}));
    assert_eq!(not_edit["ok"], false);
}
