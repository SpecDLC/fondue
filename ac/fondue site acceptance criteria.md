---
type: acceptance-criteria
status: draft
build: 1ac300ee
source: https://github.com/SpecDLC/fondue
grade: design-first conjecture
tldr: "Durable acceptance criteria for the visitor-facing Fondue page: sections, plain kitchen words, Cook mode, and an agent-drivable API. Specifications name this file, then retire. This file stays."
---

# Fondue site acceptance criteria

Status: draft. Not ratified. These criteria were written before the build, from Patrick's review of https://fondue.specdlc.com/ on 2026-10-19. No build satisfies them yet. They extend the existing cards FONDUE-SITE-1 to FONDUE-SITE-4 in this folder, which stay in force and are not restated here.

The work order that introduced this set is [[2026-10-19-fondue-site-0.3]]. That specification retires when its delivery conditions hold. This file stays. Later specifications amend it by id.

## How these age

Acceptance criteria are the contract. A specification is a unit of work that gets criteria implemented, amended or retired. Sharpening a criterion after a refutation is allowed when it does not shrink what the criterion forbids, or when Patrick records a change of intent. Weakening a criterion only to turn a red falsifier green is not a revision of the contract.

## Auxiliaries

Conditions the criteria rely on. A failure caused by a broken auxiliary is recorded as an auxiliary failure, not a refutation.

| Id | Condition |
|---|---|
| AUX-1 | `./build.sh` succeeds with the pinned toolchain and writes one self-contained page (FONDUE-SITE-1). |
| AUX-2 | Headless Chromium can open the built page at 390×844 and 1440×900. |
| AUX-3 | For WebMCP checks only: Chrome with `chrome://flags/#enable-webmcp-testing` on, or the page served with a valid WebMCP origin-trial token. Without it, SITE-7's WebMCP half is indeterminate, not refuted. |
| AUX-4 | The seed log is the one in `seed/events.json` at the bound build. |

## Acceptance convention

One stable failing observation refutes a criterion. A flaky or timed-out run is indeterminate; it is not rerun until green. Falsifiers observe the built page through a browser (DOM text, `window.fondue`, `document.modelContext`, screenshots) and through the engine's JSON API. They do not import the page's script internals.

## Criteria

### Sections

#### FONDUE-SITE-5: Four sections, Cook first

- **Claim.** The page has four sections reachable by hash: `#cook`, `#recipe`, `#compare`, `#engine`. Each renders on its own without the others' content on screen. Opening `/` with no hash shows `#cook`.
- **Interface.** The page in a browser; visible DOM text per route.
- **Forbids.** A cold load of `/` landing anywhere but Cook. The event log or cook.lock visible outside `#engine`.
- **Permits.** The agent strip (SITE-8) on every section.
- **Falsifier.** Load each route at 1440 px. Record visible text. The log's `#064`-style lines and lockfile paths appear only on `#engine`. `/` shows Cook.
- **Auxiliaries.** AUX-1, AUX-2.

#### FONDUE-SITE-6: Home-kitchen words outside Under the hood

- **Claim.** On `#cook`, `#recipe` and `#compare`, when every contract check passes, no engine term or identifier is visible. The banned list is: canon, contract, dial, portal, head, lockfile, cook.lock, revision, slot, delta, concept, unchecked, any `p:`, `i:`, `c:` or `g:` id, and any 7+ character hex hash. Words used instead are home-kitchen words, for example "Cook by" for canon, "Other ways to make it" for portal, "Exactly what you used" for cook.lock.
- **Interface.** Visible DOM text on those three routes, with the seed log and the default profile.
- **Forbids.** Any banned term or id in visible text on those routes in the passing state.
- **Permits.** Banned terms anywhere on `#engine`. A plain-language failure reason when a check fails ("Not really béchamel: no milk"). Recipe authors' own wording inside step text.
- **Falsifier.** Load each route, take `innerText` of the visible section, match it against the banned list (case-insensitive, word-bounded) and the id and hash patterns. Any hit refutes.
- **Auxiliaries.** AUX-1, AUX-2, AUX-4.

### Cooking

#### FONDUE-SITE-9: Cook mode works on a phone

- **Claim.** In Cook, after starting a recipe, the current step's text, its timer control, its "Ready when" cue when one exists, and the next step's verb are all visible at 390×844 without scrolling. The page requests a screen wake lock while a cook is in progress, and releases it when the cook ends.
- **Interface.** Screenshot and DOM geometry at 390×844; `navigator.wakeLock.request` calls observed by a stub.
- **Forbids.** Horizontal scroll at 390 px on any section. Any of the four Cook elements below the fold.
- **Permits.** Graceful absence of wake lock where the API does not exist.
- **Falsifier.** Start the seed mac and cheese, advance to the roux step, check bounding boxes against the viewport, check `scrollWidth <= clientWidth`, check the wake-lock stub was called once.
- **Auxiliaries.** AUX-1, AUX-2.

#### FONDUE-SITE-10: A recipe can be loaded, and a bad one leaves no trace

- **Claim.** `load_recipe` accepts a recipe as JSON: `id`, `title`, optional `serves`, `slots` (each `id`, `role`, `concept`, `qty`, optional `bind`) and `steps` (each `verb`, `text`, `mins`, `attend`, `uses`, `cue`, optional `why`). After it, `get_sheet` for that id lists every slot (role, qty, chosen ingredient) and every step (verb, text, mins, attend, cue) given, in order. Malformed input is refused with an error naming the bad field, and the event count is unchanged.
- **Interface.** `window.fondue.load_recipe`, `window.fondue.get_sheet`, and the engine's `events` count.
- **Forbids.** A loaded recipe missing a given slot or step. A refused recipe that appended any event (FONDUE-ENG-2 applies).
- **Permits.** New ingredient nodes created for unknown names, authored by the cook. Extra fields in `get_sheet` output.
- **Falsifier.** Load a chicken noodle soup recipe as JSON; compare. Load a recipe with a step missing `verb`; check the error and the event count.
- **Auxiliaries.** AUX-1, AUX-4.

### Agents

#### FONDUE-SITE-7: One agent API, two doors

- **Claim.** These tools exist as functions on `window.fondue`: `list_recipes`, `get_sheet`, `get_state`, `load_recipe`, `choose`, `set_pantry`, `start_cook`, `next_step`, `prev_step`, `start_timer`, `stop_timer`, `log_cook`, `publish_fork`, `reset_log`. When `document.modelContext` exists, the same tools are registered there with the same names and input schemas, and the same input produces the same result and the same appended events through either door. `publish_fork` and `reset_log` carry `consequentialHint: true`; the `get_*` and `list_*` tools carry `readOnlyHint: true`.
- **Interface.** `window.fondue`, `document.modelContext.getTools()` and `executeTool()`, the engine's `events`.
- **Forbids.** A tool on one door and not the other. Different events from the two doors for the same input. A read-only tool that appends an event. The page throwing when neither `document.modelContext` nor `navigator.modelContext` exists.
- **Permits.** Registering on `navigator.modelContext` only when `document.modelContext` is absent.
- **Falsifier.** On a fresh seed, call each tool through `window.fondue`, record the result and the new events, reset, repeat through `executeTool`, compare. Separately load the page with no model-context object and check there is no error.
- **Auxiliaries.** AUX-1, AUX-3 for the WebMCP half.

#### FONDUE-SITE-8: Everything an agent does is visible and undoable

- **Claim.** Every state change a tool makes appends at least one event. Within 250 ms of the call returning, the agent strip shows one plain-language line for it. That line's Undo appends a compensating event, after which `get_sheet` and `get_state` equal what they returned before the tool call. Undo never removes or rewrites an earlier event.
- **Interface.** `window.fondue`, the agent strip's DOM, `events`.
- **Forbids.** A state change with no event. A tool call with no strip line. An Undo that leaves the sheet or state different from before, or that shrinks the log.
- **Permits.** Read-only tools showing no strip line.
- **Falsifier.** For each state-changing tool: snapshot `get_sheet` and `get_state`, call it, wait 250 ms, find the line, click Undo, compare snapshots, check the event count only grew.
- **Auxiliaries.** AUX-1, AUX-2.

#### FONDUE-SITE-11: Consequential actions need a person

- **Claim.** `publish_fork` and `reset_log` do nothing until a person confirms on the page. Called by an agent, they return `{"status":"needs_confirmation"}` and show a confirm control; the effect happens only after a real click on it.
- **Interface.** `window.fondue`, the page's DOM, `events`.
- **Forbids.** A publish or reset that appends its events without a click on the confirm control.
- **Permits.** The human's own Publish or Reset buttons acting with one click.
- **Falsifier.** Call each through `window.fondue`; check the return, that no events were appended, and that a confirm control is visible. Click it; check the events.
- **Auxiliaries.** AUX-1, AUX-2.

### Engine

#### FONDUE-ENG-8: Adding a "why" to steps keeps old ids

- **Claim.** `Step` gains an optional `why` string. A step that does not set it serializes to the same canonical JSON as before, so every content id in the seed log is unchanged.
- **Interface.** `cargo test`, the golden id file, and the seed log's ids.
- **Forbids.** Any change to `tests/golden_id.txt` or to any id in `seed/events.json` from this change alone.
- **Falsifier.** Rebuild the seed and compare ids with the bound build; run `content_ids_are_golden`.
- **Auxiliaries.** AUX-1.
