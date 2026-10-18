# FONDUE-SITE-2 (draft)

**Claim.** Resolving the full sample dinner in the browser engine takes under 5 ms on a current laptop (the prototype measures about 0.6 ms).

**Interface.** The smoke test's timing of 200 `view` calls, and the page in a browser.

**Forbids.** A median single resolve over 5 ms in the smoke test on a CI runner.

**Done when.** The smoke test output in CI shows 200 views under 1,000 ms. (Threshold is a proposal.)
