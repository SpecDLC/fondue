# FONDUE-ENG-1 (draft)

**Claim.** Every commit's content id is computed from canonical JSON plus the schema tag `fondue/1`, and a stored log serialized to JSON and reloaded yields identical revisions, heads and lockfile.

**Interface.** `fondue-core` public API and the serialized event log.

**Forbids.** A reload that changes any revision id, head or lockfile line, or a content id that differs from the golden value.

**Done when.** `stored_log_round_trips_with_concrete_ids` and `content_ids_are_golden` pass in CI.
