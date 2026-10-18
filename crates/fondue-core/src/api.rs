//! A JSON-in, JSON-out command surface. The browser (through WASM) and native
//! tests both drive the engine through `Engine::call`.

use crate::model::*;
use crate::resolve::{diff, lock, portal, Resolver};
use crate::state::{Event, State};
use serde::Deserialize;
use serde_json::{json, Value};

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
            Cmd::Events {} => Ok(serde_json::to_value(&self.state.log).unwrap()),
            Cmd::Profile { cook } => Ok(serde_json::to_value(self.profile(&cook, None)).unwrap()),
            Cmd::Canons {} => Ok(serde_json::to_value(&self.state.canons).unwrap()),
        }
    }
}
