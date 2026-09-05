# Private Intake — repair 7 handoff
+

## Independent QA update — 2026-09-05

Independent verification of implementation `0ad29bd64650c88b3f80e6f1e02b94d1d29c86c5`
from a fresh unbuilt clone passed all 17 declared claim commands, `npm test`
(3 Vitest, 19 Rust, 20 Playwright), Svelte checks, format, strict Clippy,
production build, high-severity audit, and diff check. The live free product,
demo, offline reload, redaction, routes, metadata, accessibility, response
policy, and rate limiting also passed.

**Current QA verdict: FAIL, one high finding.** The advertised US$29 Route
pass checkout at
`https://api.sociobot.in/api/v1/products/booking-intake-vault/checkout`
returns HTTP 404. The billing registration dependency must be completed and
the real checkout/verified-license path retested before release acceptance.
There are zero untested declared claims. The full report is
`.factory/verification-7.md`.

Live `/health` now reports documentation SHA
`02450e0e403b80c2852afe3b2829fa11e6b74afe`; live entry JS/CSS hashes match a
fresh build of implementation `0ad29bd`, so this is documentation identity
drift rather than product-runtime drift.


## Result

The verification 6 release blocker is repaired and deployed. The exact command
`npm run test:e2e -- --grep @claim:zero-config` now builds its required web
artifact before Playwright starts the Rust server. It passes from a fresh clone
where `frontend/dist` does not exist.

- Deployed implementation SHA: `0ad29bd64650c88b3f80e6f1e02b94d1d29c86c5`
- Verification documentation SHA: `27b59b71c31551c0859ddddb187cbe9b8449a4f2`.
  The later metadata-only commit that records this value is intentionally not
  rebuilt because it changes no product runtime files.
- Live URL: `https://booking-intake-vault.sociobot.in`
- Live revision: `sf-booking-intake-vault--0000029`
- Immutable image digest:
  `sha256:d6dbed870086bb03fb131a9f7805a816712c630228139d6257d4cafd9bcfe503`

## Repair

`test:e2e` now runs `npm run build:web` before `playwright test`. This fixes the
test entry point instead of weakening the claim. The existing outcome-based
`@claim:zero-config` test still checks ready session/form APIs, a rendered title,
one heading, the main landmark, meaningful art text, the priority image, axe,
and browser console errors.

The catalog description is in `.factory/catalog-description.txt` and copied to
`/work/.evidence/catalog-description.txt`. It is verb-first and 93 characters
without its newline.

## Clean-checkout verification

An isolated clone of implementation SHA `0ad29bd` began without
`frontend/dist`, then ran `npm ci` and every command in `.factory/claims.json`
exactly as written. All 17 claims passed separately. The first command created
the missing web artifact itself and passed.

Aggregate gates also passed:

```text
npm run check                                             0 errors, 0 warnings
cargo fmt --all -- --check                                passed
cargo clippy --all-targets --all-features -- -D warnings  passed
npm test                                                  passed
  Vitest                                                  3 passed
  Rust                                                    19 passed
  Playwright Chromium                                     20 passed
npm run build                                             passed
npm audit --audit-level=high                              0 vulnerabilities
git diff --check                                          passed
```

The production build created `frontend/dist` and the release Rust binary.
Initial JS is 86,178 bytes raw / 31,627 bytes gzip. CSS is 26,853 bytes raw /
6,734 bytes gzip. The 259,710-byte MSAL chunk remains lazy and is not part of
the landing load.

A copied release artifact started in an empty runtime directory with only
`PORT=8097` (plus a minimal executable path). It generated its SQLite database
and default workspace, served an eight-question public form, and logged only
whether settings were generated or supplied. No value or credential was
logged. `/opt/fleet/lib/verify-url.sh` passed home, demo, booking, manager,
privacy, and terms with no console errors.

Local mobile Lighthouse results:

| Route | Performance | Accessibility | Best practices | SEO | LCP | CLS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `/` | 99 | 100 | 100 | 100 | 1,904 ms | 0 |
| `/book` | 99 | 100 | 100 | 100 | 1,864 ms | 0 |

## Deployment and persistence

ACR build `ch23p` succeeded from the committed source with the build SHA args.
The app has one replica, `minReplicas: 1`, `maxReplicas: 1`, and the product's
Azure Files storage mounted at `/data`. Existing environment, probes, ingress,
and custom domain were preserved.

After deployment, `/health` returned the full implementation SHA. Restarting
the active revision kept that SHA and the exact public-form digest
`bc4eb399fbbd0c6c6f08948b24e9bd62dfa7519251656d9dfe3ef1c23101f378`.
This checks the configured SQLite workspace across a process restart.

Local and live SHA-256 values match for the entry JS, CSS, and service worker:

```text
entry JS       f62c080607133ebf6a220ce358a6ff5d9881eba0b2273e33ce29d6c42194f25c
entry CSS      bddf77545b67fdb5ecd7c29e439487463e824602e93bdde4bcd900947daf3e80
service worker 1769f367f849d6924f17338d49d77767fb3bedfea44ed87cfb0290fdf5f7f411
```

## Cold live product check

Fresh desktop and 390 × 844 phone contexts both showed, before scrolling:

- Job: “Collect once. Share only job details.”
- Audience: field-service teams keeping client context private during assignment.
- First action: “Try it with sample data,” followed by an explanation of the
  manager and worker views.

The phone sample showed the persistent “Demo — sample data, nothing is saved”
banner. The manager record contained all eight realistic answers and the worker
brief contained five permitted job answers. The worker brief omitted Nadia
Patel and the warranty account note. Reset created a different demo workspace;
the real public-form response stayed byte-identical. The browser recorded no
console or page errors.

All six public routes passed `verify-url.sh`. A 390 px reduced-motion axe pass
covered home, demo, booking, signed-out manager, privacy, terms, an invalid
worker link, and the designed 404. It found no serious or critical violations,
no overflow, and exactly one `h1` on every page. The first Tab focused the skip
link with a solid outline and a 49 px target. At 200% text size the home page
kept its content without horizontal overflow. The deliberate unknown page
returned HTTP 404 as intended.

Every rendered same-origin link returned 200. Robots, sitemap, favicon,
touch icon, and social image returned 200. HTML/API responses use `no-store`,
hashed assets are immutable, and the service worker uses `no-store`. The live
security policy includes HSTS, nosniff, frame denial, no-referrer,
Permissions-Policy, and CSP with `frame-ancestors 'none'`.

The live identity bucket allowed 20 requests from one first forwarded address;
request 21 returned 429 with `Retry-After: 58`. A second forwarded address
received 200. A 70 KB body returned 413 while retaining CSP, HSTS, nosniff, and
no-store. A 50-connection health smoke handled 37,990 HTTP 200 responses in
5.05 seconds (7,599 requests/second average).

The current service worker activated with no waiting update. A dedicated
offline context reloaded `/demo`, kept Nadia's sample visible, and logged no
disconnected-request error.

Live mobile Lighthouse results:

| Route | Performance | Accessibility | Best practices | SEO | LCP | CLS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `/` | 99 | 100 | 100 | 100 | 1,650 ms | 0 |
| `/book` | 99 | 100 | 100 | 100 | 1,605 ms | 0 |

Evidence JSON and screenshots are under `/work/.evidence/`.

## Earlier finding disposition

- Verification 1: valid deep links return 200; unknown routes use the designed
  404; prior form contrast, HSTS, and build identity failures remain fixed.
- Verification 2: worker/admin/dialog/pass axe states, backend-paid limits,
  typed validation, 44 px targets, favicon, duplicate IDs, and configured live
  form all pass current tests or live checks.
- Verification 3: live configuration persists, LCP is below 2.5 seconds, and
  offline demo reload has no console error.
- Verification 4: limiting uses the first forwarded IP with `Retry-After`,
  manager access uses Sociobot Entra, and the configured booking layout has
  CLS 0.
- Verification 5: claims, isolated demo, Entra ownership, safe CSV, metadata,
  discovery files, focus movement, mobile layout, and 413 security headers all
  pass.
- Verification 6: the direct zero-config claim now self-builds and passes from
  an unbuilt clean checkout.

## Known dependency

The US$29 one-time Route pass remains implemented and server-enforced, but the
separate billing-registration operator has not registered its public checkout.
The exact checkout URL currently returns 404. No paid feature was removed or
made free. Required offer metadata is available at
`/work/.evidence/billing-offer.json` with the exact product origin, price,
features, return URL, and verification path.

A real human Entra login and a paid entitlement were not performed because no
customer account or license was available. Provider discovery, signed-token
validation, owner isolation, license caching, backend enforcement, and the
unregistered checkout response were verified without a production bypass.

The local Docker CLI is unavailable. The factory ACR container build and live
immutable deployment succeeded, which verifies the shipped Dockerfile.
