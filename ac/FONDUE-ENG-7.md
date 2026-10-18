# FONDUE-ENG-7 (draft)

**Claim.** Composition cycles are refused at commit (including through pinned revisions), when a head moves, and at resolve time from a cook's own choices.

**Interface.** The engine's `append` and resolve results.

**Forbids.** Any cycle that is accepted, or a resolve that loops or crashes instead of returning an error.

**Done when.** `cycles_are_refused_at_commit_including_pinned_revisions`, `cycles_are_refused_when_a_head_moves` and `resolve_time_cycles_are_caught_too` pass in CI.
