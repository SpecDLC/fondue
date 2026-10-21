# FONDUE-SITE-4

This claim was recorded on 2026-10-21.

**Claim.** The page sets no first-party cookies and keeps all recipe and cooking state in the browser. The only server-side measurement is Cloudflare Web Analytics, disclosed on the page with what it collects and why (to improve the product). Third-party requests on the hosted page are limited to web fonts and that analytics beacon. A visitor who wants no analytics can run `./build.sh` and open the self-contained page locally, where Cloudflare does not add the beacon. Fonts still load from the font host unless they are self-hosted.

**Interface.** The hosted page text, plus the browser network and storage panels on that page. A local `./build.sh` page is the check that Cloudflare does not add the beacon.

**Forbids.** Any other analytics or third-party request; recipe, pantry, or cook data leaving the browser; the disclosure missing from the hosted page.

**Done when.** The hosted page discloses Cloudflare Web Analytics (what it collects and why) and the local-build escape, and it does not say that nothing is stored on a server. A missing disclosure fails.
