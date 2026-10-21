//! Loading a recipe from plain JSON, and the cook's session in the log.

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

fn events(e: &mut Engine) -> usize {
    run(e, json!({"op": "events"}))["data"].as_array().unwrap().len()
}

fn soup() -> Value {
    json!({
        "id": "chicken-noodle-soup",
        "title": "Chicken Noodle Soup",
        "serves": "4-6",
        "slots": [
            { "id": "fat", "role": "fat", "concept": "c:cooking-fat", "qty": "2 Tbsp", "bind": "i:butter-unsalted" },
            { "id": "mirepoix", "role": "aromatics", "concept": "c:mirepoix", "qty": "2 cups" }
        ],
        "steps": [
            { "verb": "SWEAT", "text": "Mirepoix in the fat until soft.", "mins": 8, "attend": false, "uses": ["fat", "mirepoix"], "cue": "onion is clear, not browned", "why": "Soft vegetables sweeten the broth." },
            { "verb": "SIMMER", "text": "Add broth and noodles.", "mins": 9, "attend": true, "uses": [], "cue": "" }
        ]
    })
}

#[test]
fn a_recipe_loads_and_shows_every_slot_and_step() {
    let mut e = engine();
    let before = events(&mut e);
    let r = run(&mut e, json!({"op": "import", "recipe": soup(), "cook": "patrick"}));
    assert_eq!(r["ok"], true, "{}", r);
    assert_eq!(r["data"]["node"], "p:chicken-noodle-soup");
    assert_eq!(r["data"]["created"], json!(["c:mirepoix"]));
    let v = run(&mut e, json!({"op": "view", "root": "p:chicken-noodle-soup", "cook": "patrick"}));
    let t = &v["data"]["tree"];
    assert_eq!(t["name"], "Chicken Noodle Soup");
    assert_eq!(t["yields"], "4-6");
    let slots: Vec<&str> = t["slots"].as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(slots, ["fat", "mirepoix"]);
    assert_eq!(t["slots"][0]["child"]["node"], "i:butter-unsalted");
    let steps = t["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["verb"], "SWEAT");
    assert_eq!(steps[1]["attend"], true);
    assert_eq!(events(&mut e), before + r["data"]["events"].as_u64().unwrap() as usize);
}

#[test]
fn a_bad_recipe_names_the_field_and_leaves_no_trace() {
    let cases: Vec<(Box<dyn Fn(&mut Value)>, &str)> = vec![
        (Box::new(|r| { r["steps"][1].as_object_mut().unwrap().remove("verb"); }), "steps[1].verb"),
        (Box::new(|r| r["steps"][0]["mins"] = json!("eight")), "steps[0].mins"),
        (Box::new(|r| r["steps"][0]["uses"] = json!(["nope"])), "steps[0].uses[0]"),
        (Box::new(|r| { r.as_object_mut().unwrap().remove("title"); }), "title"),
        (Box::new(|r| r["slots"][1]["concept"] = json!("mirepoix")), "slots[1].concept"),
        (Box::new(|r| r["slots"][0]["bind"] = json!("i:oat-milk")), "slots[0].bind"),
        (Box::new(|r| r["slots"][1]["id"] = json!("fat")), "slots[1].id"),
        (Box::new(|r| r["id"] = json!("mac-and-cheese-nana")), "id"),
    ];
    for (mutate, field) in cases {
        let mut e = engine();
        let before = events(&mut e);
        let mut bad = soup();
        mutate(&mut bad);
        let r = run(&mut e, json!({"op": "import", "recipe": bad, "cook": "patrick"}));
        assert_eq!(r["ok"], false, "{} should be refused", field);
        assert!(r["error"].as_str().unwrap().contains(field), "{} not named in {}", field, r["error"]);
        assert_eq!(events(&mut e), before, "{} left events behind", field);
        let missing = run(&mut e, json!({"op": "view", "root": "c:mirepoix", "cook": "patrick"}));
        assert_eq!(missing["ok"], false, "{} left a node behind", field);
    }
}

#[test]
fn loading_again_is_safe_and_changes_make_a_new_revision() {
    let mut e = engine();
    run(&mut e, json!({"op": "import", "recipe": soup(), "cook": "patrick"}));
    let n = events(&mut e);
    let again = run(&mut e, json!({"op": "import", "recipe": soup(), "cook": "patrick"}));
    assert_eq!(again["data"]["unchanged"], true);
    assert_eq!(events(&mut e), n);
    let mut changed = soup();
    changed["steps"][1]["mins"] = json!(12);
    let r = run(&mut e, json!({"op": "import", "recipe": changed, "cook": "patrick"}));
    assert_eq!(r["data"]["unchanged"], false);
    assert!(r["data"]["prev_rev"].is_string());
    assert_eq!(events(&mut e), n + 1);
    // Back to the first text, then to the second again: both are accepted.
    let back = run(&mut e, json!({"op": "import", "recipe": soup(), "cook": "patrick"}));
    assert_eq!(back["ok"], true, "{}", back);
    let forth = run(&mut e, json!({"op": "import", "recipe": changed, "cook": "patrick"}));
    assert_eq!(forth["ok"], true, "{}", forth);
    let v = run(&mut e, json!({"op": "view", "root": "p:chicken-noodle-soup", "cook": "patrick"}));
    assert_eq!(v["data"]["tree"]["steps"][1]["mins"], 12.0);
}

#[test]
fn a_session_replays_and_a_bad_one_is_refused() {
    let mut e = engine();
    run(&mut e, json!({"op": "import", "recipe": soup(), "cook": "patrick"}));
    let s = json!({"recipe": "p:chicken-noodle-soup", "active": true, "step": 1, "serve_at": "6:30 PM",
        "timers": [{"id": "t1", "step": 1, "label": "noodles", "mins": 9.5, "at": 1760900000000u64}], "retired": []});
    let ok = run(&mut e, json!({"op": "append", "event": {"type": "session", "cook": "patrick", "session": s}}));
    assert_eq!(ok["ok"], true, "{}", ok);
    assert_eq!(run(&mut e, json!({"op": "session", "cook": "patrick"}))["data"], s);
    // The stored log replays to the same session.
    let log = run(&mut e, json!({"op": "events"}))["data"].clone();
    let mut f = Engine::default();
    assert_eq!(run(&mut f, json!({"op": "load", "events": log}))["ok"], true);
    assert_eq!(run(&mut f, json!({"op": "session", "cook": "patrick"}))["data"], s);
    let n = events(&mut e);
    let bad = run(&mut e, json!({"op": "append", "event": {"type": "session", "cook": "patrick", "session": {"recipe": "p:nope"}}}));
    assert_eq!(bad["ok"], false);
    assert_eq!(events(&mut e), n);
}

#[test]
fn a_cook_log_can_be_taken_back_once_and_the_log_only_grows() {
    let mut e = engine();
    let avg = |e: &mut Engine| run(e, json!({"op": "view", "root": "p:bechamel-nana", "cook": "patrick"}))["data"]["tree"]["outcome_n"].as_u64().unwrap();
    let n0 = avg(&mut e);
    let seq = events(&mut e);
    run(&mut e, json!({"op": "append", "event": {"type": "cook_log", "cook": "patrick", "node": "p:bechamel-nana", "rev": "^", "rating": 5}}));
    assert_eq!(avg(&mut e), n0 + 1);
    let r = run(&mut e, json!({"op": "append", "event": {"type": "retract", "seq": seq, "by": "patrick"}}));
    assert_eq!(r["ok"], true, "{}", r);
    assert_eq!(avg(&mut e), n0);
    assert_eq!(events(&mut e), seq + 2);
    let twice = run(&mut e, json!({"op": "append", "event": {"type": "retract", "seq": seq, "by": "patrick"}}));
    assert_eq!(twice["ok"], false);
    let not_a_rating = run(&mut e, json!({"op": "append", "event": {"type": "retract", "seq": 0, "by": "patrick"}}));
    assert_eq!(not_a_rating["ok"], false);
}

#[test]
fn a_step_why_is_stored_and_shown() {
    let mut e = engine();
    let r = run(&mut e, json!({"op": "import", "recipe": soup(), "cook": "patrick"}));
    assert_eq!(r["ok"], true, "{}", r);
    let v = run(&mut e, json!({"op": "view", "root": "p:chicken-noodle-soup", "cook": "patrick"}));
    let steps = &v["data"]["tree"]["steps"];
    assert_eq!(steps[0]["why"], "Soft vegetables sweeten the broth.");
    assert!(steps[1].get("why").map_or(true, |w| w == ""), "a step with no why shows none: {}", steps[1]);
    let bad = run(&mut e, json!({"op": "import", "recipe": { "id": "x", "title": "X", "serves": "1", "slots": [], "steps": [{"verb": "A", "text": "b", "mins": 1, "attend": false, "uses": [], "why": 3}]}, "cook": "patrick"}));
    assert_eq!(bad["ok"], false);
}
