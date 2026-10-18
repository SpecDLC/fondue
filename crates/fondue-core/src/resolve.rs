//! Resolution: turn a node plus a cook's profile into a concrete tree.
//! This is where the load-bearing rules live:
//!   - levels are derived from nesting, never stored
//!   - slots bind by choice > pantry > recipe default > canon suggestion
//!   - contracts are checked against what the bound child *is*, per the cook's canons and dial
//!   - binding deltas drop slots and steps, and add steps
//!   - known components fold, but ▲ attend flags still bubble up

use crate::model::*;
use crate::state::State;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Check {
    /// unchecked | ok | deviates | violates | blocked
    pub status: String,
    pub canon: Option<String>,
    pub must_fail: Vec<String>,
    pub should_fail: Vec<String>,
}

impl Check {
    fn unchecked() -> Check {
        Check { status: "unchecked".into(), canon: None, must_fail: vec![], should_fail: vec![] }
    }
}

/// A ▲ that bubbles up: which preparation it comes from, and the verb.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Attend {
    pub from: String,
    pub verb: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RSlot {
    pub id: String,
    pub role: String,
    pub concept: NodeId,
    pub qty: String,
    /// choice | pantry | recipe | canon:<id> | unbound
    pub via: String,
    /// Bound to something other than the recipe author's default.
    pub substituted: bool,
    pub dropped: bool,
    pub needs_choice: bool,
    /// Candidates refused, with reasons.
    pub blocked: Vec<String>,
    pub check: Check,
    pub child: Option<Box<Resolved>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Resolved {
    pub node: NodeId,
    pub rev: RevId,
    pub kind: String,
    pub name: String,
    /// Derived: ingredient | component | dish | meal | event
    pub level: String,
    pub depth: usize,
    pub implements: Vec<NodeId>,
    pub facts: Facts,
    pub folded: bool,
    pub slots: Vec<RSlot>,
    pub steps: Vec<Step>,
    pub deltas: Vec<String>,
    pub own_mins: f64,
    pub total_mins: f64,
    pub attend: Vec<Attend>,
    pub author: String,
    pub forked_from: Option<Pin>,
    pub published: bool,
    /// Ratings on this revision.
    pub outcome_n: usize,
    pub outcome_avg: f64,
    /// Ratings across every revision of the node.
    pub node_outcome_n: usize,
    pub node_outcome_avg: f64,
    pub labels: Vec<String>,
    pub brand: Option<String>,
    pub yields: String,
}

pub struct Resolver<'a> {
    pub st: &'a State,
    pub p: &'a Profile,
    stack: Vec<NodeId>,
}

impl<'a> Resolver<'a> {
    pub fn new(st: &'a State, p: &'a Profile) -> Self {
        Resolver { st, p, stack: vec![] }
    }

    /// Which revision of `node` this cook sees: their own head, then each
    /// subscribed canon's head, then main, then the newest revision.
    pub fn head_for(&self, node: &str) -> Option<RevId> {
        let cook_scope = format!("cook:{}", self.p.cook);
        if let Some(r) = self.st.head(&cook_scope, node) {
            return Some(r.clone());
        }
        for c in &self.p.canons {
            if let Some(r) = self.st.head(&format!("canon:{}", c), node) {
                return Some(r.clone());
            }
        }
        self.st.head("main", node).or_else(|| self.st.tip(node)).cloned()
    }

    pub fn resolve_root(&mut self, node: &str) -> Result<Resolved, String> {
        let rev = self.head_for(node).ok_or_else(|| format!("unknown node {}", node))?;
        self.resolve(node, &rev, None, 0)
    }

    pub fn resolve(&mut self, node: &str, rev: &str, parent_kind: Option<&str>, depth: usize) -> Result<Resolved, String> {
        if self.stack.iter().any(|n| n == node) {
            let mut path = self.stack.clone();
            path.push(node.into());
            return Err(format!("composition cycle at resolve: {}", path.join(" → ")));
        }
        let st = self.st;
        let r = st.revs.get(rev).ok_or_else(|| format!("unknown rev {}", rev))?;
        if r.node != node {
            return Err(format!("rev {} is not a revision of {}", rev, node));
        }
        self.stack.push(node.into());
        let out = self.resolve_inner(r, parent_kind, depth);
        self.stack.pop();
        out
    }

    fn resolve_inner(&mut self, r: &Revision, parent_kind: Option<&str>, depth: usize) -> Result<Resolved, String> {
        let (n, avg) = self.st.outcome_rev(&r.id);
        let (nn, navg) = self.st.outcome_node(&r.node);
        let labels = self.st.labels.iter().filter(|l| l.node == r.node).map(|l| format!("{} ({})", l.label, l.by)).collect();
        let mut out = Resolved {
            node: r.node.clone(),
            rev: r.id.clone(),
            kind: r.body.kind().into(),
            name: r.body.name().into(),
            level: String::new(),
            depth,
            implements: r.body.implements().to_vec(),
            facts: Facts::new(),
            folded: false,
            slots: vec![],
            steps: vec![],
            deltas: vec![],
            own_mins: 0.0,
            total_mins: 0.0,
            attend: vec![],
            author: r.author.clone(),
            forked_from: r.forked_from.clone(),
            published: self.st.published.contains(&r.id),
            outcome_n: n,
            outcome_avg: avg,
            node_outcome_n: nn,
            node_outcome_avg: navg,
            labels,
            brand: None,
            yields: String::new(),
        };

        match &r.body {
            Body::Concept(_) => out.level = "concept".into(),
            Body::Item(b) => {
                out.level = "ingredient".into();
                out.facts = b.facts.clone();
                out.brand = b.brand.clone();
            }
            Body::Preparation(b) => {
                out.level = if parent_kind == Some("preparation") { "component".into() } else { "dish".into() };
                out.yields = b.yields.clone();
                out.folded = depth > 0 && b.implements.iter().any(|c| self.p.known.iter().any(|k| self.st.concept_is_a(c, k)));
                out.slots = self.bind_slots(&r.node, &b.slots, "preparation", depth)?;
                let (steps, deltas) = apply_variants(b, &mut out.slots);
                out.steps = steps;
                out.deltas = deltas;

                for (k, v) in &b.facts {
                    out.facts.insert(k.clone(), fact_value(v, &out.slots));
                }
                out.own_mins = out.steps.iter().map(|s| s.mins).sum();
                let child_mins: f64 = live_children(&out.slots).map(|c| c.total_mins).sum();
                // Known simplification: children run in sequence. Lanes come later.
                out.total_mins = out.own_mins + child_mins;
                out.attend = out.steps.iter().filter(|s| s.attend).map(|s| Attend { from: out.name.clone(), verb: s.verb.clone() }).collect();
                let bubbled: Vec<Attend> = live_children(&out.slots).flat_map(|c| c.attend.clone()).collect();
                out.attend.extend(bubbled);
            }
            Body::Gathering(b) => {
                out.slots = self.bind_slots(&r.node, &b.slots, "gathering", depth)?;
                out.level = if live_children(&out.slots).any(|c| c.kind == "gathering") { "event".into() } else { "meal".into() };
                out.yields = b.serve.clone().unwrap_or_default();
                // Dishes in a meal run in parallel: the meal takes as long as its longest dish.
                out.total_mins = live_children(&out.slots).map(|c| c.total_mins).fold(0.0, f64::max);
                out.attend = live_children(&out.slots).flat_map(|c| c.attend.clone()).collect();
            }
        }
        Ok(out)
    }

    fn bind_slots(&mut self, recipe: &str, slots: &[Slot], kind: &str, depth: usize) -> Result<Vec<RSlot>, String> {
        let st = self.st;
        let mut out = vec![];
        for s in slots {
            let key = format!("{}/{}", recipe, s.id);
            let mut cands: Vec<(String, Pin)> = vec![];
            if let Some(n) = self.p.choices.get(&key) {
                cands.push(("choice".into(), Pin { node: n.clone(), rev: None }));
            }
            if let Some(n) = self.pantry_for(s) {
                cands.push(("pantry".into(), Pin { node: n, rev: None }));
            }
            if let Some(b) = &s.bind {
                cands.push(("recipe".into(), b.clone()));
            }
            for c in &self.p.canons {
                if let Some(pin) = st.canons.get(c).and_then(|cn| cn.prefer.get(&s.concept)) {
                    cands.push((format!("canon:{}", c), pin.clone()));
                }
            }
            // The same node offered twice is one candidate.
            let mut seen: BTreeSet<Pin> = BTreeSet::new();
            cands.retain(|(_, p)| seen.insert(p.clone()));

            let mut rs = RSlot {
                id: s.id.clone(),
                role: s.role.clone(),
                concept: s.concept.clone(),
                qty: s.qty.clone(),
                via: "unbound".into(),
                substituted: false,
                dropped: false,
                needs_choice: false,
                blocked: vec![],
                check: Check::unchecked(),
                child: None,
            };
            for (via, pin) in cands {
                let rev = match &pin.rev {
                    Some(r) if st.revs.get(r).map(|x| x.node == pin.node).unwrap_or(false) => r.clone(),
                    Some(r) => {
                        rs.blocked.push(format!("{}@{}: no such revision", pin.node, r));
                        continue;
                    }
                    None => match self.head_for(&pin.node) {
                        Some(r) => r,
                        None => {
                            rs.blocked.push(format!("{}: unknown node", pin.node));
                            continue;
                        }
                    },
                };
                let body = &st.revs[&rev].body;
                if !st.implements(body, &s.concept) {
                    rs.blocked.push(format!("{} does not implement {}", body.name(), s.concept));
                    continue;
                }
                let child = self.resolve(&pin.node, &rev, Some(kind), depth + 1)?;
                let check = self.check_child(&s.concept, &child);
                if check.status == "blocked" {
                    rs.blocked.push(format!("{} ({}: {})", child.name, check.canon.clone().unwrap_or_default(), check.must_fail.join(", ")));
                    continue;
                }
                rs.substituted = s.bind.as_ref().map(|b| b.node != pin.node).unwrap_or(false);
                rs.via = via;
                rs.check = check;
                rs.child = Some(Box::new(child));
                break;
            }
            // Ask once: nothing bound, or only a canon's suggestion filled a slot
            // the recipe author left open. The cook confirms or picks.
            if rs.child.is_none() || (rs.via.starts_with("canon:") && s.bind.is_none()) {
                rs.needs_choice = true;
            }
            out.push(rs);
        }
        Ok(out)
    }

    /// The cook's pantry entry for this slot, if it should apply.
    /// Entries are keyed by what the cook keeps ("butter") and apply to slots
    /// asking for it or anything it extends ("cooking fat"); the most specific
    /// entry wins, ties broken by key so the result never depends on map order.
    /// A pantry entry never overrides an author's default that is a different
    /// kind of thing: keeping butter does not turn an olive-oil roux into butter.
    fn pantry_for(&self, s: &Slot) -> Option<NodeId> {
        let st = self.st;
        let mut entries: Vec<(usize, &NodeId, &NodeId)> = self
            .p
            .pantry
            .iter()
            .filter(|(c, _)| st.concept_is_a(c, &s.concept))
            .filter(|(c, _)| match &s.bind {
                None => true,
                Some(pin) => st.tip(&pin.node).map(|r| st.implements(&st.revs[r].body, c)).unwrap_or(true),
            })
            .map(|(c, n)| (st.concept_chain(c).len(), c, n))
            .collect();
        entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        entries.first().map(|(_, _, n)| (*n).clone())
    }

    /// Check a bound child against the contracts for every concept it is:
    /// the slot's concept and everything the child implements, with all their
    /// ancestors. A butter in a "cooking fat" slot still answers to butter's rules.
    pub fn check_child(&self, slot_concept: &str, child: &Resolved) -> Check {
        let mut concepts: Vec<NodeId> = self.st.concept_chain(slot_concept);
        for c in &child.implements {
            for x in self.st.concept_chain(c) {
                if !concepts.contains(&x) {
                    concepts.push(x);
                }
            }
        }
        self.check_concepts(&concepts, &child.facts)
    }

    /// Check facts as an implementation of one concept (and its ancestors).
    pub fn check(&self, concept: &str, facts: &Facts) -> Check {
        self.check_concepts(&self.st.concept_chain(concept), facts)
    }

    /// The first subscribed canon with any contract for these concepts decides.
    fn check_concepts(&self, concepts: &[NodeId], facts: &Facts) -> Check {
        if self.p.dial == Dial::Off {
            return Check::unchecked();
        }
        for cid in &self.p.canons {
            let canon = match self.st.canons.get(cid) {
                Some(c) => c,
                None => continue,
            };
            let contracts: Vec<&Contract> = concepts.iter().filter_map(|c| canon.contracts.get(c)).collect();
            if contracts.is_empty() {
                continue;
            }
            let must_fail: Vec<String> = contracts.iter().flat_map(|c| c.must.iter()).filter(|r| !rule_ok(r, facts)).map(|r| r.label.clone()).collect();
            let should_fail: Vec<String> = contracts.iter().flat_map(|c| c.should.iter()).filter(|r| !rule_ok(r, facts)).map(|r| r.label.clone()).collect();
            let status = if !must_fail.is_empty() {
                if self.p.dial == Dial::Enforce { "blocked" } else { "violates" }
            } else if !should_fail.is_empty() {
                "deviates"
            } else {
                "ok"
            };
            return Check { status: status.into(), canon: Some(cid.clone()), must_fail, should_fail };
        }
        Check::unchecked()
    }
}

fn live_children(slots: &[RSlot]) -> impl Iterator<Item = &Resolved> {
    slots.iter().filter(|s| !s.dropped).filter_map(|s| s.child.as_deref())
}

/// "@slot:<id>.<fact>" reads a fact from whatever is bound to that slot.
fn fact_value(v: &Value, slots: &[RSlot]) -> Value {
    if let Value::String(s) = v {
        if let Some(rest) = s.strip_prefix("@slot:") {
            if let Some((sid, fact)) = rest.split_once('.') {
                return slots
                    .iter()
                    .find(|x| x.id == sid && !x.dropped)
                    .and_then(|x| x.child.as_ref())
                    .and_then(|c| c.facts.get(fact).cloned())
                    .unwrap_or(Value::Null);
            }
        }
    }
    v.clone()
}

/// Apply binding deltas.
///   1. Variants are evaluated in order. A variant whose trigger slot an
///      earlier variant dropped does not fire.
///   2. Steps using any dropped slot are removed.
///   3. Every `at` is an index into the list *after the drops*; inserts are
///      applied from the highest index down, so they never shift each other.
fn apply_variants(b: &PrepBody, slots: &mut [RSlot]) -> (Vec<Step>, Vec<String>) {
    let mut dropped: Vec<String> = vec![];
    let mut inserts: Vec<(usize, usize, Vec<Step>)> = vec![]; // (at, order, steps)
    let mut notes = vec![];
    for (order, v) in b.variants.iter().enumerate() {
        if dropped.contains(&v.when.slot) {
            continue;
        }
        let hit = slots
            .iter()
            .find(|s| s.id == v.when.slot)
            .and_then(|s| s.child.as_ref())
            .map(|c| c.facts.get(&v.when.fact) == Some(&v.when.eq))
            .unwrap_or(false);
        if !hit {
            continue;
        }
        dropped.extend(v.delta.drop_slots.iter().cloned());
        if !v.delta.add_steps.is_empty() {
            inserts.push((v.delta.at.unwrap_or(usize::MAX), order, v.delta.add_steps.clone()));
        }
        notes.push(v.delta.note.clone());
    }
    for s in slots.iter_mut() {
        if dropped.contains(&s.id) {
            s.dropped = true;
        }
    }
    let mut steps: Vec<Step> = b.steps.iter().filter(|st| !st.uses.iter().any(|u| dropped.contains(u))).cloned().collect();
    let base_len = steps.len();
    // Highest index first; for equal indices, later variants first so earlier ones end up first.
    inserts.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    for (at, _, add) in inserts {
        let i = at.min(base_len);
        for (k, s) in add.into_iter().enumerate() {
            steps.insert(i + k, s);
        }
    }
    (steps, notes)
}

pub fn rule_ok(r: &Rule, facts: &Facts) -> bool {
    let f = match facts.get(&r.fact) {
        Some(v) if !v.is_null() => v,
        // An absent fact satisfies "not equal to" and nothing else.
        _ => return r.op == Op::Ne,
    };
    let num = |v: &Value| v.as_f64();
    match r.op {
        Op::Eq => f == &r.value,
        Op::Ne => f != &r.value,
        Op::Gte => matches!((num(f), num(&r.value)), (Some(a), Some(b)) if a >= b),
        Op::Lte => matches!((num(f), num(&r.value)), (Some(a), Some(b)) if a <= b),
        Op::Between => {
            let lo = r.value.get(0).and_then(num);
            let hi = r.value.get(1).and_then(num);
            matches!((num(f), lo, hi), (Some(x), Some(a), Some(b)) if x >= a && x <= b)
        }
    }
}

// ---------- projections built on top of resolution ----------

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LockLine {
    pub path: String,
    pub node: NodeId,
    pub rev: RevId,
    pub via: String,
}

/// Every resolved pin, in a stable order. Today this is a report of what was
/// resolved; resolving *from* a lock (reproducible cooks) is not built yet.
pub fn lock(root: &Resolved) -> Vec<LockLine> {
    let mut out = vec![LockLine { path: short(&root.node), node: root.node.clone(), rev: root.rev.clone(), via: "root".into() }];
    fn walk(r: &Resolved, prefix: &str, out: &mut Vec<LockLine>) {
        for s in &r.slots {
            if s.dropped {
                continue;
            }
            if let Some(c) = &s.child {
                let path = format!("{}/{}", prefix, s.id);
                out.push(LockLine { path: path.clone(), node: c.node.clone(), rev: c.rev.clone(), via: s.via.clone() });
                walk(c, &path, out);
            }
        }
    }
    walk(root, &short(&root.node), &mut out);
    out
}

fn short(n: &str) -> String {
    n.split_once(':').map(|(_, b)| b.to_string()).unwrap_or_else(|| n.to_string())
}

#[derive(Clone, Debug, Serialize)]
pub struct PortalEntry {
    pub node: NodeId,
    pub rev: RevId,
    pub name: String,
    pub kind: String,
    pub author: String,
    pub forked_from: Option<Pin>,
    pub published: bool,
    pub outcome_n: usize,
    pub outcome_avg: f64,
    pub node_outcome_n: usize,
    pub node_outcome_avg: f64,
    pub labels: Vec<String>,
    pub preferred_by: Vec<String>,
    pub check: Check,
    pub facts: Facts,
    pub total_mins: f64,
    pub brand: Option<String>,
}

/// Every implementation of a concept, as this cook would see it.
/// Note: resolves every candidate in full; fine for a demo, needs an index later.
pub fn portal(st: &State, p: &Profile, concept: &str) -> Vec<PortalEntry> {
    let mut out = vec![];
    for node in st.node_revs.keys() {
        let mut r = Resolver::new(st, p);
        let rev = match r.head_for(node) {
            Some(x) => x,
            None => continue,
        };
        if !st.implements(&st.revs[&rev].body, concept) {
            continue;
        }
        let res = match r.resolve(node, &rev, Some("preparation"), 1) {
            Ok(x) => x,
            Err(_) => continue,
        };
        let check = r.check_child(concept, &res);
        let preferred_by = st.canons.values().filter(|c| c.prefer.get(concept).map(|pin| &pin.node == node).unwrap_or(false)).map(|c| c.id.clone()).collect();
        out.push(PortalEntry {
            node: node.clone(),
            rev,
            name: res.name.clone(),
            kind: res.kind.clone(),
            author: res.author.clone(),
            forked_from: res.forked_from.clone(),
            published: res.published,
            outcome_n: res.outcome_n,
            outcome_avg: res.outcome_avg,
            node_outcome_n: res.node_outcome_n,
            node_outcome_avg: res.node_outcome_avg,
            labels: res.labels.clone(),
            preferred_by,
            check,
            facts: res.facts.clone(),
            total_mins: res.total_mins,
            brand: res.brand.clone(),
        });
    }
    let first = p.canons.first().cloned().unwrap_or_default();
    let ord = |x: f64, y: f64| y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal);
    out.sort_by(|a, b| {
        let ka = (a.check.status == "blocked", !a.preferred_by.contains(&first));
        let kb = (b.check.status == "blocked", !b.preferred_by.contains(&first));
        ka.cmp(&kb)
            .then(ord(a.outcome_avg, b.outcome_avg))
            .then(ord(a.node_outcome_avg, b.node_outcome_avg))
            .then(b.outcome_n.cmp(&a.outcome_n))
            .then(a.name.cmp(&b.name))
    });
    out
}

/// What changed between two revisions, in kitchen terms.
pub fn diff(st: &State, a: &str, b: &str) -> Result<Vec<String>, String> {
    let ra = st.revs.get(a).ok_or_else(|| format!("unknown rev {}", a))?;
    let rb = st.revs.get(b).ok_or_else(|| format!("unknown rev {}", b))?;
    let mut out = vec![];
    if ra.body.kind() != rb.body.kind() {
        out.push(format!("kind {} → {}", ra.body.kind(), rb.body.kind()));
        return Ok(out);
    }
    if ra.body.name() != rb.body.name() {
        out.push(format!("name “{}” → “{}”", ra.body.name(), rb.body.name()));
    }
    let sa: BTreeMap<&str, &Slot> = ra.body.slots().iter().map(|s| (s.id.as_str(), s)).collect();
    let sb: BTreeMap<&str, &Slot> = rb.body.slots().iter().map(|s| (s.id.as_str(), s)).collect();
    for (id, s) in &sa {
        match sb.get(id) {
            None => out.push(format!("slot {} removed ({})", id, s.role)),
            Some(t) => {
                if s.qty != t.qty {
                    out.push(format!("{}: {} → {}", s.role, s.qty, t.qty));
                }
                if s.bind != t.bind {
                    let f = |p: &Option<Pin>| p.as_ref().map(|x| x.node.clone()).unwrap_or_else(|| "unbound".into());
                    out.push(format!("{} binds {} → {}", s.role, f(&s.bind), f(&t.bind)));
                }
            }
        }
    }
    for (id, t) in &sb {
        if !sa.contains_key(id) {
            out.push(format!("slot {} added ({} {})", id, t.qty, t.role));
        }
    }
    if let (Body::Preparation(pa), Body::Preparation(pb)) = (&ra.body, &rb.body) {
        let key = |s: &Step| format!("{} — {}", s.verb, s.text);
        let ka: Vec<String> = pa.steps.iter().map(key).collect();
        let kb: Vec<String> = pb.steps.iter().map(key).collect();
        for k in &ka {
            if !kb.contains(k) {
                out.push(format!("− step {}", k));
            }
        }
        for k in &kb {
            if !ka.contains(k) {
                out.push(format!("+ step {}", k));
            }
        }
        for (k, v) in &pb.facts {
            if pa.facts.get(k) != Some(v) {
                out.push(format!("{}: {} → {}", k, pa.facts.get(k).map(|x| x.to_string()).unwrap_or_else(|| "—".into()), v));
            }
        }
        let ma: f64 = pa.steps.iter().map(|s| s.mins).sum();
        let mb: f64 = pb.steps.iter().map(|s| s.mins).sum();
        if (ma - mb).abs() > 0.01 {
            out.push(format!("own time {} → {} min", ma, mb));
        }
    }
    if out.is_empty() {
        out.push("no changes".into());
    }
    Ok(out)
}
