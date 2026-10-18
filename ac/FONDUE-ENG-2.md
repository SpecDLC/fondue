# FONDUE-ENG-2 (draft)

**Claim.** Typos, duplicate commits and bad pins are refused with an error, and a refused event changes nothing in state.

**Interface.** The engine's JSON call interface (`append`) and the resulting state.

**Forbids.** A malformed event that is accepted, silently dropped, or leaves any trace in heads, revisions or logs.

**Done when.** `malformed_events_are_refused`, `duplicate_commits_are_refused_not_dropped`, `bad_pins_are_refused` and `rejected_events_leave_no_trace` pass in CI.
