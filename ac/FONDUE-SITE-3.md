# FONDUE-SITE-3 (draft, proposed)

**Claim.** `https://fondue.specdlc.com/` serves the demo page with a valid certificate, deployed only from `main` of the public repository.

**Interface.** An HTTPS request to the URL and the deploy workflow history.

**Forbids.** A production deploy from any branch other than `main`, or the URL serving anything but the latest merged release.

**Done when.** After the first merged release, the URL returns 200 with the page and the certificate validates.
