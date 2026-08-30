# Verification 6 — Private Intake

**Result: FAIL (release blocked)**

Verified on 2026-08-30 against candidate commit
`08cf309f67edf801c4f53128f2ee085d41c454af` and
`https://booking-intake-vault.sociobot.in`.

## Release-blocking finding

### High — required clean-clone claim command fails before a web build

The required manifest exists and was read before other QA. From the clean
checkout, after `npm ci` but before any web build, its exact command

```sh
npm run test:e2e -- --grep @claim:zero-config
```

failed. Playwright started `cargo run` successfully, but the server returned
an empty document instead of the client shell:

```text
Expected pattern: /Private Intake/
Received string:  ""
tests/product.spec.ts:14: await expect(page).toHaveTitle(/Private Intake/)
```

`frontend/dist` is generated and not present in a clean clone. The claim test
starts the Rust server directly, but its command does not first build that
client artifact. This violates the claims contract: every listed exact command
must pass from a clean clone through the product entry point. It is explicitly
release-blocking under the work order, even though the container image and the
already deployed service include a built client.

Repair the test entry point (for example, make its setup build the web client
before the server starts) and rerun every command in `.factory/claims.json`
from a freshly cloned, unbuilt worktree.

## First-read test — live cold load

**Pass.** A fresh desktop browser load showed:

- What: “Collect once. Share only job details.” and the explanatory text says
  it is a private booking flow.
- For whom: “For field-service teams that keep client context private when
  they assign work.”
- First action: the visible, one-click **Try it with sample data** link,
  accompanied by “See one booking split into manager and worker views.”

The action opened `/demo`, showed the persistent “Demo — sample data, nothing
is saved” banner, manager and worker views, **Reset demo**, and Start for
real. The worker sample contained the permitted address/job details but not
`Nadia Patel` or `Warranty account NW-204`.

## Claims and local quality gates

The three direct Rust claim commands passed:

```text
cargo test tests::claim_automatic_deletion                         1 passed
cargo test tests::tokens_are_never_stored_verbatim                 1 passed
cargo test tests::durable_snapshot_restores_the_last_local_vault_copy  1 passed
```

After `npm run build:web` supplied the missing generated artifact, the full
suite passed and exercised every browser claim tag (including demo isolation,
server redaction, link expiry, CSV safety, tracker log, offline reload, Entra,
paid boundary, rate limit, policy headers, metadata, build identity and mobile
keyboard):

```text
npm test
Vitest: 3 passed
Rust:   19 passed
Playwright Chromium: 20 passed (30.7 s)
```

This later pass is useful regression evidence, but does not erase the failed
required clean-clone claim invocation above.

Other completed local gates:

```text
npm run check                                            0 errors, 0 warnings
cargo fmt --all -- --check                               passed
cargo clippy --all-targets --all-features -- -D warnings  passed
npm run build                                             passed (web + release Rust binary)
npm audit --audit-level=high                              0 vulnerabilities
```

The production build emitted initial entry JS 86,178 bytes raw / 31.73 KB
gzip and CSS 26,853 bytes raw / 6.69 KB gzip. The 259,710-byte MSAL chunk is
separate from the first landing load. Docker was unavailable in this verifier
container (`docker: command not found`), so the Docker image could not be
independently built here.

## Independent live verification

- **Candidate identity:** `/health` returned
  `{"build":"08cf309f67edf801c4f53128f2ee085d41c454af","status":"ok"}`.
  SHA-256 of the live entry JS and CSS exactly matched locally built
  `frontend/dist` assets (`f62c…f25c` and `bddf…f80` respectively).
- **Routes and headers:** `/`, `/demo`, `/book`, `/admin`, `/privacy`, and
  `/terms` returned 200; a made-up route returned the designed 404. HTML and
  API responses had `no-store`; hashed JS/CSS had
  `public, max-age=31536000, immutable`. HTTPS responses include CSP with
  `frame-ancestors 'none'`, HSTS, nosniff, Referrer-Policy, and
  Permissions-Policy.
- **Privacy:** a cold landing and complete demo/reset request log contained
  only `https://booking-intake-vault.sociobot.in`. The only manager sign-in
  authority requested after explicit click was
  `https://sociobotcustomers.ciamlogin.com/.../.well-known/openid-configuration`.
  No real Entra account was used.
- **Rate limit:** 20 `/api/session` requests with one first
  `X-Forwarded-For` address returned 200; request 21 returned 429 with
  `Retry-After: 59`; a different first address immediately returned 200.
- **Desktop/mobile/accessibility:** Playwright found no console/page errors
  and axe found no serious or critical violations on the landing page. At a
  390 px viewport, `/book` had `scrollWidth == clientWidth == 390`, no visible
  link target was under 44 px, and Tab reached the skip link with a solid
  focus outline. Reduced-motion media matched and the shipped CSS reduces
  transition/animation duration to `.01ms`.
- **Offline/PWA:** the passing browser claim creates a dedicated context,
  confirms `/sw.js` control, updates the service worker, goes offline, and
  reloads `/demo` with Nadia's sample still visible.

## Manual product paths covered by the passing suite

The browser/API tests cover a representative public booking, private manager
field and worker-safe field, manager preview and actual expiring worker link,
deletion/revocation, malformed date and phone (422 recovery), free-tier ninth
field and 14-day-link limits (402), duplicate IDs (422), 70 KB body policy
(413 with security headers), and CSV formula neutralization. The backend tests
also cover first-boot workspace configuration, server-only redaction, hashed
tokens, retention purge, durable snapshot restoration, owner isolation, and
per-IP API limiting.

## Retest condition

Do not release until every command in `.factory/claims.json`, especially the
exact `@claim:zero-config` command, passes on a newly cloned checkout without
an existing `frontend/dist` directory. Then rerun this verification.
