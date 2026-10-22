//! The event log and the projection folded from it. Nothing here is edited in
//! place: every change is an appended event, and `State` is rebuilt by replay.
//!
//! Events may be written with revision aliases; `apply` resolves them and the
//! log stores the concrete event, so replay never depends on alias semantics.
//!   "^"      the node's main head (its newest revision if main has none)
//!   "@tip"   the node's newest revision
//!   "@first" the node's first revision

use crate::model::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Bump when the hashed shape of a revision changes. Part of every content id.
pub const SCHEMA: &str = "fondue/1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    /// Add a revision to a node. The revision id is a content hash.
    Commit {
        node: NodeId,
        #[serde(default)]
        parent: Option<RevId>,
        author: String,
        #[serde(default)]
        msg: String,
        body: Body,
        #[serde(default)]
        forked_from: Option<Pin>,
    },
    /// Point a scope's head at a revision. Scopes: "main", "canon:<id>", "cook:<id>".
    Head {
        scope: String,
        node: NodeId,
        rev: RevId,
        #[serde(default)]
        by: String,
    },
    /// Publish or replace a canon definition.
    Canon { canon: Canon, author: String },
    /// Replace a cook's whole profile (bulk import). Prefer `Pref` for edits.
    Profile {
        profile: Profile,
        #[serde(default)]
        by: String,
    },
    /// Change one field of a cook's profile. Two devices editing different
    /// fields no longer overwrite each other.
    Pref { cook: String, set: PrefOp },
    /// An outcome: someone cooked a revision and rated it.
    CookLog {
        cook: String,
        node: NodeId,
        rev: RevId,
        rating: u8,
        #[serde(default)]
        note: String,
    },
    /// Evidence or a decorator attached without changing the revision.
    Label { node: NodeId, rev: RevId, label: String, by: String },
    /// Make a revision visible in the public portal.
    Publish { node: NodeId, rev: RevId, by: String },
    /// Where a cook is in the kitchen right now: the open recipe, the step they
    /// are on, running timers. Replaces the cook's whole session, so undoing a
    /// move is appending the earlier session. `Pref` stays for lasting settings.
    Session { cook: String, session: Session },
    /// Take back a `CookLog` by its position in the log. The log itself is never
    /// edited: this event removes the outcome from the projection.
    Retract { seq: usize, by: String },
    /// Names one edit, made by hand or by an agent, and the events that put it
    /// back. Appended right after the edit's own events. `after` is filled in by
    /// the engine: the value each undo event's field held once the edit was made,
    /// so a later Undo can tell whether that field has changed since.
    Edit {
        by: String,
        text: String,
        #[serde(default)]
        undo: Vec<Event>,
        #[serde(default)]
        after: Vec<Value>,
    },
    /// Reverse the edit recorded at `undoes` by applying its undo events. One
    /// labeled event; the log only grows. Refused if that edit was already
    /// undone, or if any field it set has changed since.
    Undo { undoes: usize, by: String },
}

fn field_name(ev: &Event) -> &'static str {
    match ev {
        Event::Pref { set: PrefOp::Pantry { .. }, .. } => "that pantry item",
        Event::Pref { set: PrefOp::Choose { .. }, .. } => "that pick",
        Event::Pref { set: PrefOp::Dial { .. }, .. } => "the rules setting",
        Event::Pref { set: PrefOp::Canons { .. }, .. } => "the house style",
        Event::Pref { set: PrefOp::Know { .. }, .. } => "that \"I know this\" setting",
        Event::Session { .. } => "the cook in progress",
        Event::Head { .. } => "that recipe's version",
        Event::Retract { .. } => "that rating",
        _ => "something it changed",
    }
}

/// A cook's moment-to-moment state. Not a kitchen setting.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Session {
    /// The recipe currently open.
    #[serde(default)]
    pub recipe: Option<NodeId>,
    /// A cook is in progress.
    #[serde(default)]
    pub active: bool,
    /// Zero-based position in the flattened step sequence.
    #[serde(default)]
    pub step: usize,
    /// Free text the cook gave ("6:30 PM"). Stored, never computed on.
    #[serde(default)]
    pub serve_at: Option<String>,
    #[serde(default)]
    pub timers: Vec<Timer>,
    /// Recipes the cook took back; hidden from the catalog views.
    #[serde(default)]
    pub retired: Vec<NodeId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Timer {
    pub id: String,
    pub step: usize,
    pub label: String,
    pub mins: f64,
    /// Epoch milliseconds when it started. The caller supplies it: the engine has no clock.
    pub at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum PrefOp {
    Dial { dial: Dial },
    Canons { canons: Vec<String> },
    /// `node: null` clears the pantry entry.
    Pantry { concept: NodeId, #[serde(default)] node: Option<NodeId> },
    Know { concept: NodeId, on: bool },
    /// `node: null` clears the choice.
    Choose { key: String, #[serde(default)] node: Option<NodeId> },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CookLogRec {
    pub cook: String,
    pub node: NodeId,
    pub rev: RevId,
    pub rating: u8,
    pub note: String,
    pub seq: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LabelRec {
    pub node: NodeId,
    pub rev: RevId,
    pub label: String,
    pub by: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct State {
    /// The log as stored: aliases already resolved to concrete ids.
    pub log: Vec<Event>,
    pub revs: BTreeMap<RevId, Revision>,
    /// Revisions per node, in log order.
    pub node_revs: BTreeMap<NodeId, Vec<RevId>>,
    /// scope -> node -> rev
    pub heads: BTreeMap<String, BTreeMap<NodeId, RevId>>,
    pub canons: BTreeMap<String, Canon>,
    pub profiles: BTreeMap<String, Profile>,
    pub logs: Vec<CookLogRec>,
    pub labels: Vec<LabelRec>,
    pub published: BTreeSet<RevId>,
    pub sessions: BTreeMap<String, Session>,
    /// Seqs of `Edit` records that have been undone.
    pub undone: BTreeSet<usize>,
}

// ---------- content ids ----------

fn fnv1a(bytes: &[u8], basis: u64) -> u64 {
    let mut h = basis;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Drop nulls, empty strings and empty containers, so adding a new optional
/// field with an empty default does not change existing ids.
fn prune(v: &Value) -> Option<Value> {
    match v {
        Value::Null => None,
        Value::String(s) if s.is_empty() => None,
        Value::Array(a) => {
            // Array positions are meaningful (steps, slots): keep pruned entries as null.
            if a.is_empty() {
                None
            } else {
                Some(Value::Array(a.iter().map(|x| prune(x).unwrap_or(Value::Null)).collect()))
            }
        }
        Value::Object(m) => {
            let out: serde_json::Map<String, Value> = m.iter().filter_map(|(k, x)| prune(x).map(|p| (k.clone(), p))).collect();
            if out.is_empty() {
                None
            } else {
                Some(Value::Object(out))
            }
        }
        other => Some(other.clone()),
    }
}

/// Canonical JSON: keys sorted explicitly (independent of serde_json's
/// `preserve_order` feature), no whitespace.
fn write_canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(m) => {
            out.push('{');
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push(':');
                write_canonical(&m[*k], out);
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(x, out);
            }
            out.push(']');
        }
        other => out.push_str(&serde_json::to_string(other).unwrap()),
    }
}

/// A short content id for any JSON value (used for the seed id).
pub fn digest(v: &Value) -> String {
    let c = canonical_json(v);
    format!("{:016x}{:016x}", fnv1a(c.as_bytes(), 0xcbf29ce484222325), fnv1a(c.as_bytes(), 0x84222325cbf29ce4))
}

pub fn canonical_json(v: &Value) -> String {
    let mut s = String::new();
    write_canonical(&prune(v).unwrap_or(Value::Null), &mut s);
    s
}

/// 128-bit id from two FNV-1a passes. A placeholder for blake3; the canonical
/// encoding and schema tag are the parts meant to last.
pub fn content_id(node: &str, parent: &Option<RevId>, author: &str, msg: &str, body: &Body, forked: &Option<Pin>) -> RevId {
    let v = serde_json::json!({
        "schema": SCHEMA, "node": node, "parent": parent, "author": author,
        "msg": msg, "body": body, "forked_from": forked,
    });
    let c = canonical_json(&v);
    format!("{:016x}{:016x}", fnv1a(c.as_bytes(), 0xcbf29ce484222325), fnv1a(c.as_bytes(), 0x84222325cbf29ce4))
}

impl State {
    pub fn from_events(events: Vec<Event>) -> Result<State, String> {
        let mut s = State::default();
        for (i, e) in events.into_iter().enumerate() {
            s.apply(e).map_err(|err| format!("event {}: {}", i, err))?;
        }
        Ok(s)
    }

    pub fn tip(&self, node: &str) -> Option<&RevId> {
        self.node_revs.get(node).and_then(|v| v.last())
    }

    pub fn head(&self, scope: &str, node: &str) -> Option<&RevId> {
        self.heads.get(scope).and_then(|m| m.get(node))
    }

    /// Divergent tips: revisions of a node that no other revision names as parent.
    pub fn tips(&self, node: &str) -> Vec<RevId> {
        let revs = match self.node_revs.get(node) {
            Some(v) => v,
            None => return vec![],
        };
        let parents: BTreeSet<&RevId> = revs.iter().filter_map(|r| self.revs[r].parent.as_ref()).collect();
        revs.iter().filter(|r| !parents.contains(r)).cloned().collect()
    }

    /// Resolve "^", "@tip", "@first"; check a concrete id belongs to `node`.
    pub fn resolve_rev(&self, node: &str, rev: &str) -> Result<RevId, String> {
        let id = match rev {
            "^" => self.head("main", node).or_else(|| self.tip(node)).cloned(),
            "@tip" => self.tip(node).cloned(),
            "@first" => self.node_revs.get(node).and_then(|v| v.first()).cloned(),
            other => Some(other.to_string()),
        }
        .ok_or_else(|| format!("{} has no revisions", node))?;
        match self.revs.get(&id) {
            Some(r) if r.node == node => Ok(id),
            Some(r) => Err(format!("rev {} belongs to {}, not {}", id, r.node, node)),
            None => Err(format!("unknown rev {} for {}", id, node)),
        }
    }

    fn resolve_pin(&self, p: &Pin) -> Result<Pin, String> {
        if !self.node_revs.contains_key(&p.node) {
            return Err(format!("pin to unknown node {}", p.node));
        }
        Ok(Pin { node: p.node.clone(), rev: match &p.rev { Some(r) => Some(self.resolve_rev(&p.node, r)?), None => None } })
    }

    fn resolve_body_pins(&self, body: &Body) -> Result<Body, String> {
        let mut b = body.clone();
        let slots: Option<&mut Vec<Slot>> = match &mut b {
            Body::Preparation(p) => Some(&mut p.slots),
            Body::Gathering(g) => Some(&mut g.slots),
            _ => None,
        };
        if let Some(slots) = slots {
            for s in slots.iter_mut() {
                if let Some(p) = &s.bind {
                    s.bind = Some(self.resolve_pin(p)?);
                }
            }
        }
        Ok(b)
    }

    /// Apply one event. On error nothing changes. Returns a short description.
    pub fn apply(&mut self, ev: Event) -> Result<String, String> {
        let seq = self.log.len();
        match ev {
            Event::Edit { by, text, undo, .. } => {
                if text.trim().is_empty() {
                    return Err("edit: text is missing".into());
                }
                if undo.iter().any(|u| matches!(u, Event::Edit { .. } | Event::Undo { .. })) {
                    return Err("edit: undo events cannot be edits or undos".into());
                }
                let after = undo.iter().map(|u| self.probe(u)).collect();
                let out = format!("edit by {}: {}", by, text);
                self.log.push(Event::Edit { by, text, undo, after });
                return Ok(out);
            }
            Event::Undo { undoes, by } => {
                let (text, undo) = self.check_undo(undoes)?;
                // Apply the undo events on a scratch copy; keep the projection, not their log entries.
                let mut next = self.clone();
                for u in undo {
                    next.apply(u).map_err(|e| format!("This can't be undone: {}. Nothing was changed.", e))?;
                }
                next.log.truncate(seq);
                next.undone.insert(undoes);
                *self = next;
                self.log.push(Event::Undo { undoes, by });
                return Ok(format!("undo #{}: {}", undoes, text));
            }
            ev => self.apply_plain(ev, seq),
        }
    }

    /// The current value of the field an undo event would write, or null for
    /// events that write no single field.
    pub fn probe(&self, ev: &Event) -> Value {
        use serde_json::json;
        match ev {
            Event::Pref { cook, set } => {
                let p = self.profiles.get(cook).cloned().unwrap_or_else(|| Profile { cook: cook.clone(), ..Default::default() });
                match set {
                    PrefOp::Dial { .. } => json!(p.dial),
                    PrefOp::Canons { .. } => json!(p.canons),
                    PrefOp::Pantry { concept, .. } => json!(p.pantry.get(concept)),
                    PrefOp::Know { concept, .. } => json!(p.known.contains(concept)),
                    PrefOp::Choose { key, .. } => json!(p.choices.get(key)),
                }
            }
            Event::Session { cook, .. } => json!(self.sessions.get(cook).cloned().unwrap_or_default()),
            Event::Head { scope, node, .. } => json!(self.heads.get(scope).and_then(|m| m.get(node))),
            Event::Retract { seq, .. } => json!(self.logs.iter().any(|l| l.seq == *seq)),
            _ => Value::Null,
        }
    }

    /// Why the edit at `seq` can't be undone now, in plain words, or its text and undo events.
    pub fn check_undo(&self, seq: usize) -> Result<(String, Vec<Event>), String> {
        let (text, undo, after) = match self.log.get(seq) {
            Some(Event::Edit { text, undo, after, .. }) => (text, undo, after),
            _ => return Err(format!("#{} is not an edit that can be undone", seq)),
        };
        if undo.is_empty() {
            return Err(format!("\"{}\" can't be undone.", text));
        }
        if self.undone.contains(&seq) {
            return Err(format!("\"{}\" was already undone.", text));
        }
        for (u, a) in undo.iter().zip(after.iter()) {
            if &self.probe(u) != a {
                return Err(format!("\"{}\" can't be undone: {} has changed since. Nothing was changed.", text, field_name(u)));
            }
        }
        Ok((text.clone(), undo.clone()))
    }

    fn apply_plain(&mut self, ev: Event, seq: usize) -> Result<String, String> {
        // Resolve aliases first; `stored` is what goes in the log.
        let stored = match ev {
            Event::Commit { node, parent, author, msg, body, forked_from } => {
                let parent = match parent {
                    Some(p) => Some(self.resolve_rev(&node, &p)?),
                    None => None,
                };
                let body = self.resolve_body_pins(&body)?;
                let forked_from = match forked_from {
                    Some(f) => Some(self.resolve_pin(&f)?),
                    None => None,
                };
                Event::Commit { node, parent, author, msg, body, forked_from }
            }
            Event::Head { scope, node, rev, by } => Event::Head { rev: self.resolve_rev(&node, &rev)?, scope, node, by },
            Event::CookLog { cook, node, rev, rating, note } => Event::CookLog { rev: self.resolve_rev(&node, &rev)?, cook, node, rating, note },
            Event::Label { node, rev, label, by } => Event::Label { rev: self.resolve_rev(&node, &rev)?, node, label, by },
            Event::Publish { node, rev, by } => Event::Publish { rev: self.resolve_rev(&node, &rev)?, node, by },
            Event::Session { cook, session } => {
                if let Some(r) = &session.recipe {
                    if !self.node_revs.contains_key(r) {
                        return Err(format!("session names unknown recipe {}", r));
                    }
                }
                if session.timers.iter().any(|t| t.mins < 0.0 || !t.mins.is_finite()) {
                    return Err("timer mins must be a non-negative number".into());
                }
                Event::Session { cook, session }
            }
            other => other,
        };

        let out = match &stored {
            Event::Commit { node, parent, author, msg, body, forked_from } => {
                if let Some(first) = self.node_revs.get(node).and_then(|v| v.first()) {
                    let k = self.revs[first].body.kind();
                    if k != body.kind() {
                        return Err(format!("{} is a {}; a revision cannot make it a {}", node, k, body.kind()));
                    }
                }
                if parent.is_none() && self.node_revs.contains_key(node) {
                    return Err(format!("{} already exists; a new revision needs a parent", node));
                }
                self.check_no_cycle(node, body)?;
                let id = content_id(node, parent, author, msg, body, forked_from);
                if self.revs.contains_key(&id) {
                    return Err(format!("{}@{} already exists (identical commit)", node, short(&id)));
                }
                let main_head = self.head("main", node).cloned();
                let advances_main = main_head.is_none() || main_head.as_ref() == parent.as_ref();
                self.revs.insert(
                    id.clone(),
                    Revision { id: id.clone(), node: node.clone(), parent: parent.clone(), author: author.clone(), msg: msg.clone(), body: body.clone(), forked_from: forked_from.clone(), seq },
                );
                self.node_revs.entry(node.clone()).or_default().push(id.clone());
                if advances_main {
                    self.heads.entry("main".into()).or_default().insert(node.clone(), id.clone());
                    format!("commit {}@{}", node, short(&id))
                } else {
                    format!("commit {}@{} (diverges; main stays at {})", node, short(&id), short(main_head.as_deref().unwrap_or("")))
                }
            }
            Event::Head { scope, node, rev, .. } => {
                // Moving a head changes what un-pinned slots resolve to, so it can close a loop too.
                let body = self.revs[rev].body.clone();
                let mut trial = self.clone();
                trial.heads.entry(scope.clone()).or_default().insert(node.clone(), rev.clone());
                trial.check_no_cycle(node, &body)?;
                self.heads.entry(scope.clone()).or_default().insert(node.clone(), rev.clone());
                format!("head {} {} -> {}", scope, node, short(rev))
            }
            Event::Canon { canon, .. } => {
                self.canons.insert(canon.id.clone(), canon.clone());
                format!("canon {}", canon.id)
            }
            Event::Profile { profile, .. } => {
                self.profiles.insert(profile.cook.clone(), profile.clone());
                format!("profile {}", profile.cook)
            }
            Event::Pref { cook, set } => {
                let p = self.profiles.entry(cook.clone()).or_insert_with(|| Profile { cook: cook.clone(), ..Default::default() });
                match set {
                    PrefOp::Dial { dial } => p.dial = *dial,
                    PrefOp::Canons { canons } => p.canons = canons.clone(),
                    PrefOp::Pantry { concept, node } => match node {
                        Some(n) => {
                            p.pantry.insert(concept.clone(), n.clone());
                        }
                        None => {
                            p.pantry.remove(concept);
                        }
                    },
                    PrefOp::Know { concept, on } => {
                        if *on {
                            p.known.insert(concept.clone());
                        } else {
                            p.known.remove(concept);
                        }
                    }
                    PrefOp::Choose { key, node } => match node {
                        Some(n) => {
                            p.choices.insert(key.clone(), n.clone());
                        }
                        None => {
                            p.choices.remove(key);
                        }
                    },
                }
                format!("pref {} {:?}", cook, set)
            }
            Event::CookLog { cook, node, rev, rating, note } => {
                if !(1..=5).contains(rating) {
                    return Err("rating must be 1–5".into());
                }
                self.logs.push(CookLogRec { cook: cook.clone(), node: node.clone(), rev: rev.clone(), rating: *rating, note: note.clone(), seq });
                format!("cook_log {} rated {}@{} {}", cook, node, short(rev), rating)
            }
            Event::Label { node, rev, label, by } => {
                self.labels.push(LabelRec { node: node.clone(), rev: rev.clone(), label: label.clone(), by: by.clone() });
                format!("label {}@{} {}", node, short(rev), label)
            }
            Event::Publish { node, rev, .. } => {
                self.published.insert(rev.clone());
                format!("publish {}@{}", node, short(rev))
            }
            Event::Session { cook, session } => {
                self.sessions.insert(cook.clone(), session.clone());
                format!("session {}", cook)
            }
            Event::Retract { seq, .. } => {
                if !matches!(self.log.get(*seq), Some(Event::CookLog { .. })) {
                    return Err(format!("#{} is not a cook_log", seq));
                }
                let before = self.logs.len();
                self.logs.retain(|l| l.seq != *seq);
                if self.logs.len() == before {
                    return Err(format!("cook_log #{} was already taken back", seq));
                }
                format!("retract cook_log #{}", seq)
            }
            Event::Edit { .. } | Event::Undo { .. } => unreachable!("handled in apply"),
        };
        self.log.push(stored);
        Ok(out)
    }

    /// Every revision an un-pinned or pinned reference to `pin` could resolve
    /// to, for any cook: the pinned rev, or every scope's head plus the tip.
    fn candidates(&self, pin: &Pin) -> Vec<RevId> {
        if let Some(r) = &pin.rev {
            return vec![r.clone()];
        }
        let mut v: BTreeSet<RevId> = self.heads.values().filter_map(|m| m.get(&pin.node).cloned()).collect();
        if let Some(t) = self.tip(&pin.node) {
            v.insert(t.clone());
        }
        v.into_iter().collect()
    }

    /// Composition must stay a DAG for every cook. Follows pinned revisions
    /// and every scope's head, so it is conservative: if any cook could see a
    /// loop through recipe-author pins, the commit or head move is refused.
    /// (Loops a cook closes with their own choices are caught at resolve.)
    fn check_no_cycle(&self, node: &str, body: &Body) -> Result<(), String> {
        let mut stack: Vec<(Pin, Vec<NodeId>)> = body.slots().iter().filter_map(|s| s.bind.clone()).map(|p| { let n = p.node.clone(); (p, vec![node.to_string(), n]) }).collect();
        let mut seen: BTreeSet<RevId> = BTreeSet::new();
        while let Some((pin, path)) = stack.pop() {
            if pin.node == node {
                return Err(format!("composition cycle: {}", path.join(" → ")));
            }
            for rid in self.candidates(&pin) {
                if !seen.insert(rid.clone()) {
                    continue;
                }
                if let Some(r) = self.revs.get(&rid) {
                    for s in r.body.slots() {
                        if let Some(p) = &s.bind {
                            let mut np = path.clone();
                            np.push(p.node.clone());
                            stack.push((p.clone(), np));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Does `body` implement `concept`, directly or through `extends`?
    pub fn implements(&self, body: &Body, concept: &str) -> bool {
        body.implements().iter().any(|c| self.concept_is_a(c, concept))
    }

    pub fn concept_is_a(&self, c: &str, target: &str) -> bool {
        self.concept_chain(c).iter().any(|x| x == target)
    }

    /// `c` followed by everything it extends.
    pub fn concept_chain(&self, c: &str) -> Vec<NodeId> {
        let mut out = vec![];
        let mut cur = Some(c.to_string());
        while let Some(x) = cur {
            if out.contains(&x) || out.len() > 32 {
                break;
            }
            out.push(x.clone());
            cur = self.tip(&x).and_then(|r| match &self.revs[r].body {
                Body::Concept(cb) => cb.extends.clone(),
                _ => None,
            });
        }
        out
    }

    /// (count, average) of ratings on one revision.
    pub fn outcome_rev(&self, rev: &str) -> (usize, f64) {
        avg(self.logs.iter().filter(|l| l.rev == rev).map(|l| l.rating))
    }

    /// (count, average) of ratings across all revisions of a node.
    pub fn outcome_node(&self, node: &str) -> (usize, f64) {
        avg(self.logs.iter().filter(|l| l.node == node).map(|l| l.rating))
    }
}

fn avg(it: impl Iterator<Item = u8>) -> (usize, f64) {
    let rs: Vec<u8> = it.collect();
    if rs.is_empty() {
        (0, 0.0)
    } else {
        (rs.len(), rs.iter().map(|r| *r as f64).sum::<f64>() / rs.len() as f64)
    }
}

pub fn short(id: &str) -> &str {
    &id[..id.len().min(7)]
}
