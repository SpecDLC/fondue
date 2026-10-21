# FONDUE-SITE-12 (draft)

**Claim.** Every edit on the page can be undone the same way, whoever made it. Edits made by hand on the page and edits made through `window.fondue` share one undo path. One Undo reverses the most recent edit that has not been undone, so that `get_sheet` and `get_state` equal what they were before that edit. Undo survives a page reload, only ever appends to the log, and never silently overwrites a newer change.

**Interface.** The hosted page in a browser: its controls, the agent strip's DOM, `window.fondue` (`get_sheet`, `get_state`), the engine's `events`, and a page reload.

**Forbids.**
- An edit made by hand that offers no Undo.
- An Undo that leaves `get_sheet` or `get_state` different from before the edit it reverses.
- Undo lines that disappear after a reload, or a last edit that can no longer be undone after a reload.
- An Undo that removes or rewrites an earlier event, or an undo event that is not labeled as one (`undoes: <seq>`, naming the sequence number of the event it reverses).
- A stale Undo, one whose field has been changed since the edit, being applied silently.

**Permits.** A stale Undo shown disabled, or refused with a plain-language message that says the field has changed since. Read-only tools showing no Undo.

**Done when.** On the hosted page, after three edits (at least one made by hand and one through `window.fondue`), one click on Undo reverses the most recent edit that hasn't been undone, and `get_sheet` and `get_state` then equal what they were before that edit. This still holds after a page reload. The event count only grows, and each Undo is labeled as an undo in the log (`undoes: <seq>`). An Undo whose field has been changed since is either disabled or refused with a plain message, never applied silently.
