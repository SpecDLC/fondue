# FONDUE-ENG-5 (draft)

**Claim.** A pantry entry rebinds a slot only over a default of the same kind: keeping salted butter rebinds a butter roux and drops its added salt, but does not turn an olive-oil roux into butter.

**Interface.** The resolved view for a given profile.

**Forbids.** A pantry entry replacing a default of a different kind, or salted butter leaving the added salt in place.

**Done when.** `salted_butter_in_pantry_drops_added_salt` and `pantry_does_not_override_a_different_kind_of_default` pass in CI.
