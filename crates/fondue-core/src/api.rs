//! A JSON-in, JSON-out command surface. The browser (through WASM) and native
//! tests both drive the engine through `Engine::call`.

use crate::model::*;
use crate::resolve::{diff, lock, portal, Resolver};
use crate::state::{Event, State};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Cmd {
    /// Replace the whole log and replay it.
    Load { events: Vec<Event> },
    /// Append one event.
    Append { event: Event },
    /// Resolve a node for a cook. `profile` overrides the stored profile.
    View { root: NodeId, cook: String, #[serde(default)] profile: Option<Profile> },
    /// Every implementation of a concept, as this cook sees it.
    Portal { concept: NodeId, cook: String, #[serde(default)] profile: Option<Profile> },
    Diff { a: RevId, b: RevId },
    /// A revision's raw body, for forking.
    Get { rev: RevId },
    /// The node catalog grouped by kind.
    Catalog {},
    Events {},
    /// Turn a recipe written as plain JSON into commits, atomically.
    Import { recipe: Value, cook: String },
    /// A cook's session (empty if they have none).
    Session { cook: String },
    Profile { cook: String },
    Canons {},
}

#[derive(Default)]
pub struct Engine {
    pub state: State,
}

impl Engine {
    pub fn call(&mut self, input: &str) -> String {
        let out = match serde_json::from_str::<Cmd>(input) {
            Ok(cmd) => self.run(cmd),
            Err(e) => Err(format!("bad command: {}", e)),
        };
        match out {
            Ok(v) => json!({ "ok": true, "data": v }).to_string(),
            Err(e) => json!({ "ok": false, "error": e }).to_string(),
        }
    }

    fn profile(&self, cook: &str, over: Option<Profile>) -> Profile {
        over.or_else(|| self.state.profiles.get(cook).cloned()).unwrap_or(Profile { cook: cook.into(), ..Default::default() })
    }

    fn run(&mut self, cmd: Cmd) -> Result<Value, String> {
        match cmd {
            Cmd::Load { events } => {
                let n = events.len();
                self.state = State::from_events(events)?;
                Ok(json!({ "events": n, "revisions": self.state.revs.len() }))
            }
            Cmd::Append { event } => {
                // Validate against a scratch copy so a rejected event leaves no trace.
                let mut next = self.state.clone();
                let msg = next.apply(event)?;
                self.state = next;
                Ok(json!({ "applied": msg, "seq": self.state.log.len() - 1 }))
            }
            Cmd::View { root, cook, profile } => {
                let p = self.profile(&cook, profile);
                let mut r = Resolver::new(&self.state, &p);
                let tree = r.resolve_root(&root)?;
                let lk = lock(&tree);
                Ok(json!({ "tree": tree, "lock": lk, "profile": p }))
            }
            Cmd::Portal { concept, cook, profile } => {
                let p = self.profile(&cook, profile);
                Ok(serde_json::to_value(portal(&self.state, &p, &concept)).unwrap())
            }
            Cmd::Diff { a, b } => Ok(json!(diff(&self.state, &a, &b)?)),
            Cmd::Get { rev } => {
                let r = self.state.revs.get(&rev).ok_or_else(|| format!("unknown rev {}", rev))?;
                Ok(serde_json::to_value(r).unwrap())
            }
            Cmd::Catalog {} => {
                let mut items = vec![];
                for (node, revs) in &self.state.node_revs {
                    let tip = revs.last().unwrap();
                    let r = &self.state.revs[tip];
                    items.push(json!({
                        "node": node, "kind": r.body.kind(), "name": r.body.name(),
                        "implements": r.body.implements(), "revs": revs.len(),
                        "tips": self.state.tips(node),
                    }));
                }
                Ok(json!(items))
            }
            Cmd::Import { recipe, cook } => self.import(&recipe, &cook),
            Cmd::Session { cook } => Ok(serde_json::to_value(self.state.sessions.get(&cook).cloned().unwrap_or_default()).unwrap()),
            Cmd::Events {} => Ok(serde_json::to_value(&self.state.log).unwrap()),
            Cmd::Profile { cook } => Ok(serde_json::to_value(self.profile(&cook, None)).unwrap()),
            Cmd::Canons {} => Ok(serde_json::to_value(&self.state.canons).unwrap()),
        }
    }

    /// Validate a recipe, build every event it needs, and apply them to a copy.
    /// Only a fully applied recipe replaces the state, so a refusal appends nothing.
    /// Errors name the bad field ("steps[2].verb: missing").
    fn import(&mut self, recipe: &Value, cook: &str) -> Result<Value, String> {
        let r = parse_recipe(recipe)?;
        let node = format!("p:{}", r.id);
        let st = &self.state;
        if let Some(first) = st.node_revs.get(&node).and_then(|v| v.first()) {
            if !st.revs[first].msg.starts_with(IMPORT_MSG) {
                return Err(format!("id: {} is already a built-in recipe", r.id));
            }
            if st.revs[first].body.kind() != "preparation" {
                return Err(format!("id: {} is already in use", r.id));
            }
        }
        let mut events: Vec<Event> = vec![];
        let mut made: Vec<String> = vec![];
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut ensure = |events: &mut Vec<Event>, made: &mut Vec<String>, id: &str, body: Body, field: &str, st: &State| -> Result<(), String> {
            if st.node_revs.contains_key(id) || !seen.insert(id.to_string()) {
                return Ok(());
            }
            let _ = field;
            made.push(id.to_string());
            events.push(Event::Commit { node: id.into(), parent: None, author: cook.into(), msg: format!("{}{}", IMPORT_MSG, r.title), body, forked_from: None });
            Ok(())
        };
        let mut slots = vec![];
        for (i, s) in r.slots.iter().enumerate() {
            if let Some(k) = st.node_revs.get(&s.concept).map(|_| st.revs[&st.node_revs[&s.concept][0]].body.kind()) {
                if k != "concept" {
                    return Err(format!("slots[{}].concept: {} is a {}, not a concept", i, s.concept, k));
                }
            }
            ensure(&mut events, &mut made, &s.concept, Body::Concept(ConceptBody { name: pretty(&s.concept), extends: None, note: String::new() }), "concept", st)?;
            if let Some(b) = &s.bind {
                if let Some(first) = st.node_revs.get(b) {
                    let body = &st.revs[&first[0]].body;
                    if !matches!(body, Body::Item(_) | Body::Preparation(_)) || !st.implements(body, &s.concept) {
                        return Err(format!("slots[{}].bind: {} does not implement {}", i, b, s.concept));
                    }
                } else if b.starts_with("i:") {
                    ensure(&mut events, &mut made, b, Body::Item(ItemBody { name: pretty(b), implements: vec![s.concept.clone()], facts: Facts::new(), brand: None, source: None }), "bind", st)?;
                } else {
                    return Err(format!("slots[{}].bind: unknown recipe {}", i, b));
                }
            }
            slots.push(Slot { id: s.id.clone(), role: s.role.clone(), concept: s.concept.clone(), qty: s.qty.clone(), bind: s.bind.clone().map(|n| Pin { node: n, rev: None }) });
        }
        let body = Body::Preparation(PrepBody {
            name: r.title.clone(),
            implements: vec![],
            yields: r.serves.clone(),
            facts: Facts::new(),
            slots,
            steps: r.steps.clone(),
            variants: vec![],
        });
        let prev = st.head("main", &node).cloned();
        let same = prev.as_ref().map(|p| st.revs[p].body == body).unwrap_or(false);
        // The same text seen before (a load that was taken back, then repeated) points main at it
        // again; committing it a second time would be refused as an identical commit.
        let earlier = st.node_revs.get(&node).and_then(|v| v.iter().rev().find(|r| st.revs[*r].body == body)).cloned();
        if !same {
            match earlier {
                Some(rev) => events.push(Event::Head { scope: "main".into(), node: node.clone(), rev, by: cook.into() }),
                None => events.push(Event::Commit { node: node.clone(), parent: prev.clone().map(|_| "^".to_string()), author: cook.into(), msg: format!("{}{}", IMPORT_MSG, r.title), body, forked_from: None }),
            }
        }
        let mut next = self.state.clone();
        for e in events.iter().cloned() {
            next.apply(e)?;
        }
        let rev = next.head("main", &node).cloned().unwrap_or_default();
        let appended = next.log.len() - self.state.log.len();
        self.state = next;
        Ok(json!({ "node": node, "rev": rev, "prev_rev": prev, "created": made, "events": appended, "unchanged": same }))
    }
}

/// Marks revisions written by `import`, so a re-import may replace them but never a built-in recipe.
const IMPORT_MSG: &str = "import: ";

struct RecipeIn {
    id: String,
    title: String,
    serves: String,
    slots: Vec<SlotIn>,
    steps: Vec<Step>,
}

struct SlotIn {
    id: String,
    role: String,
    concept: String,
    qty: String,
    bind: Option<String>,
}

fn pretty(id: &str) -> String {
    let s = id.split_once(':').map(|x| x.1).unwrap_or(id).replace('-', " ");
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn field<'a>(o: &'a serde_json::Map<String, Value>, path: &str, key: &str) -> Result<&'a Value, String> {
    o.get(key).filter(|v| !v.is_null()).ok_or_else(|| format!("{}{}: missing", path, key))
}

fn text(o: &serde_json::Map<String, Value>, path: &str, key: &str, allow_empty: bool) -> Result<String, String> {
    match field(o, path, key)? {
        Value::String(s) if allow_empty || !s.trim().is_empty() => Ok(s.clone()),
        Value::String(_) => Err(format!("{}{}: must not be empty", path, key)),
        _ => Err(format!("{}{}: must be a string", path, key)),
    }
}

fn node_id(path: &str, key: &str, v: &str, prefixes: &[&str]) -> Result<(), String> {
    let ok = prefixes.iter().any(|p| v.strip_prefix(p).map(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')).unwrap_or(false));
    if ok {
        Ok(())
    } else {
        Err(format!("{}{}: {:?} must look like {}lower-case-name", path, key, v, prefixes.join(" or ")))
    }
}

fn parse_recipe(v: &Value) -> Result<RecipeIn, String> {
    let o = v.as_object().ok_or("recipe: must be a JSON object")?;
    let id = text(o, "", "id", false)?;
    let id = id.strip_prefix("p:").unwrap_or(&id).to_string();
    node_id("", "id", &format!("p:{}", id), &["p:"])?;
    let title = text(o, "", "title", false)?;
    let serves = match o.get("serves") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Err("serves: must be a string".into()),
    };
    let slots_v = field(o, "", "slots")?.as_array().ok_or("slots: must be a list")?;
    let steps_v = field(o, "", "steps")?.as_array().ok_or("steps: must be a list")?;
    if steps_v.is_empty() {
        return Err("steps: must have at least one step".into());
    }
    let mut slots: Vec<SlotIn> = vec![];
    for (i, sv) in slots_v.iter().enumerate() {
        let path = format!("slots[{}].", i);
        let so = sv.as_object().ok_or(format!("slots[{}]: must be an object", i))?;
        let sid = text(so, &path, "id", false)?;
        if slots.iter().any(|s| s.id == sid) {
            return Err(format!("{}id: {:?} is used twice", path, sid));
        }
        let concept = text(so, &path, "concept", false)?;
        node_id(&path, "concept", &concept, &["c:"])?;
        let bind = match so.get("bind") {
            None | Some(Value::Null) => None,
            Some(Value::String(b)) => {
                node_id(&path, "bind", b, &["i:", "p:"])?;
                Some(b.clone())
            }
            Some(_) => return Err(format!("{}bind: must be a string", path)),
        };
        slots.push(SlotIn { id: sid, role: text(so, &path, "role", false)?, concept, qty: text(so, &path, "qty", true)?, bind });
    }
    let mut steps = vec![];
    for (i, sv) in steps_v.iter().enumerate() {
        let path = format!("steps[{}].", i);
        let so = sv.as_object().ok_or(format!("steps[{}]: must be an object", i))?;
        let mins = field(so, &path, "mins")?.as_f64().filter(|m| *m >= 0.0 && m.is_finite()).ok_or(format!("{}mins: must be a number of minutes, 0 or more", path))?;
        let attend = field(so, &path, "attend")?.as_bool().ok_or(format!("{}attend: must be true or false", path))?;
        let uses_v = field(so, &path, "uses")?.as_array().ok_or(format!("{}uses: must be a list of slot ids", path))?;
        let mut uses = vec![];
        for (j, u) in uses_v.iter().enumerate() {
            let u = u.as_str().ok_or(format!("{}uses[{}]: must be a string", path, j))?;
            if !slots.iter().any(|s| s.id == u) {
                return Err(format!("{}uses[{}]: no slot named {:?}", path, j, u));
            }
            uses.push(u.to_string());
        }
        let why = match so.get("why") {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(w)) => w.trim().to_string(),
            Some(_) => return Err(format!("{}why: must be a string", path)),
        };
        steps.push(Step { verb: text(so, &path, "verb", false)?, text: text(so, &path, "text", false)?, mins, attend, uses, cue: text(so, &path, "cue", true)?, why });
    }
    Ok(RecipeIn { id, title, serves, slots, steps })
}
