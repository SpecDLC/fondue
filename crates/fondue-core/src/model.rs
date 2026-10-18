//! The domain model. One node type with four kinds; zoom levels are NOT stored,
//! they are derived from nesting at resolve time.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub type NodeId = String;
pub type RevId = String;
pub type Facts = BTreeMap<String, Value>;

/// A revision's content. The `kind` tag is the only intrinsic classification.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Body {
    /// An abstract "what": butter, béchamel, melting cheese. Carries no method.
    Concept(ConceptBody),
    /// A leaf with no steps: a variety, spec or product.
    Item(ItemBody),
    /// Slots plus steps. Béchamel, mac and cheese, a salad.
    Preparation(PrepBody),
    /// A meal or event: slots holding preparations, no technique of its own.
    Gathering(GatherBody),
}

impl Body {
    pub fn kind(&self) -> &'static str {
        match self {
            Body::Concept(_) => "concept",
            Body::Item(_) => "item",
            Body::Preparation(_) => "preparation",
            Body::Gathering(_) => "gathering",
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Body::Concept(b) => &b.name,
            Body::Item(b) => &b.name,
            Body::Preparation(b) => &b.name,
            Body::Gathering(b) => &b.name,
        }
    }
    pub fn implements(&self) -> &[NodeId] {
        match self {
            Body::Item(b) => &b.implements,
            Body::Preparation(b) => &b.implements,
            _ => &[],
        }
    }
    pub fn slots(&self) -> &[Slot] {
        match self {
            Body::Preparation(b) => &b.slots,
            Body::Gathering(b) => &b.slots,
            _ => &[],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConceptBody {
    pub name: String,
    /// Sub-concepts inherit the parent's contract (mornay extends béchamel).
    #[serde(default)]
    pub extends: Option<NodeId>,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ItemBody {
    pub name: String,
    pub implements: Vec<NodeId>,
    #[serde(default)]
    pub facts: Facts,
    /// Product identity when this item is a branded product.
    #[serde(default)]
    pub brand: Option<String>,
    /// Where the item definition comes from (e.g. a public standard, a dataset).
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrepBody {
    pub name: String,
    #[serde(default)]
    pub implements: Vec<NodeId>,
    #[serde(default)]
    pub yields: String,
    /// Facts may reference a bound child: "@slot:fat.fat" reads fact `fat` of
    /// whatever is bound to slot `fat`. This is how an olive-oil roux surfaces
    /// "fat = olive_oil" to a contract without anyone restating it.
    #[serde(default)]
    pub facts: Facts,
    #[serde(default)]
    pub slots: Vec<Slot>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub variants: Vec<Variant>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GatherBody {
    pub name: String,
    #[serde(default)]
    pub slots: Vec<Slot>,
    #[serde(default)]
    pub serve: Option<String>,
}

/// A role and an amount. A recipe never names a product; it declares a slot.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub id: String,
    pub role: String,
    pub concept: NodeId,
    #[serde(default)]
    pub qty: String,
    /// The author's default binding, optionally pinned to a revision.
    #[serde(default)]
    pub bind: Option<Pin>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub node: NodeId,
    #[serde(default)]
    pub rev: Option<RevId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub verb: String,
    pub text: String,
    #[serde(default)]
    pub mins: f64,
    #[serde(default)]
    pub attend: bool,
    /// Slot ids this step consumes. Dropping a slot drops its steps.
    #[serde(default)]
    pub uses: Vec<String>,
    #[serde(default)]
    pub cue: String,
}

/// A recipe-local delta triggered by what a slot is bound to.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub when: Cond,
    pub delta: Delta,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Cond {
    pub slot: String,
    pub fact: String,
    pub eq: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Delta {
    #[serde(default)]
    pub drop_slots: Vec<String>,
    #[serde(default)]
    pub add_steps: Vec<Step>,
    /// Insert added steps before this index (after the drops). Appends if absent.
    #[serde(default)]
    pub at: Option<usize>,
    #[serde(default)]
    pub note: String,
}

/// A curated, versioned set of contracts and preferences a cook subscribes to.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Canon {
    pub id: String,
    pub name: String,
    /// What the canon is based on, in our own words ("after classical technique").
    #[serde(default)]
    pub basis: String,
    #[serde(default)]
    pub contracts: BTreeMap<NodeId, Contract>,
    #[serde(default)]
    pub prefer: BTreeMap<NodeId, Pin>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    #[serde(default)]
    pub must: Vec<Rule>,
    #[serde(default)]
    pub should: Vec<Rule>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub fact: String,
    pub op: Op,
    pub value: Value,
    pub label: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    Eq,
    Ne,
    Gte,
    Lte,
    Between,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Dial {
    Off,
    #[default]
    Advise,
    Enforce,
}

/// Cook state. Lives in the event log like everything else.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub cook: String,
    /// Subscribed canons, highest priority first.
    #[serde(default)]
    pub canons: Vec<String>,
    #[serde(default)]
    pub dial: Dial,
    /// Default binding per concept ("I keep salted butter").
    #[serde(default)]
    pub pantry: BTreeMap<NodeId, NodeId>,
    /// Concepts this cook already knows; preparations implementing them fold.
    #[serde(default)]
    pub known: BTreeSet<NodeId>,
    /// "Ask once" answers, keyed "<recipe node>/<slot id>".
    #[serde(default)]
    pub choices: BTreeMap<String, NodeId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Revision {
    pub id: RevId,
    pub node: NodeId,
    pub parent: Option<RevId>,
    pub author: String,
    pub msg: String,
    pub body: Body,
    pub forked_from: Option<Pin>,
    pub seq: usize,
}
