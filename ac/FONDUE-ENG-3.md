# FONDUE-ENG-3 (draft)

**Claim.** A node's level (component, dish, meal) is derived from how it nests; the same béchamel is a component inside mac and cheese and a dish on its own.

**Interface.** The resolved view returned by the engine.

**Forbids.** A stored level field, or one node reporting the same level in two different nesting contexts that call for different levels.

**Done when.** `levels_are_derived_from_nesting` passes in CI.
