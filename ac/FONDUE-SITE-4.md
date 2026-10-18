# FONDUE-SITE-4 (draft, proposed)

**Claim.** The page sets no cookies and sends no analytics or visitor data. All engine work happens in the browser. The only third-party requests are for web fonts.

**Interface.** Browser developer tools network and storage panels while using the page.

**Forbids.** A `Set-Cookie` header, a request carrying visitor state to any server, or a third-party request other than fonts.

**Done when.** A check of the live page shows no cookies and only font requests to third parties. (Self-hosting the fonts is an open option.)
