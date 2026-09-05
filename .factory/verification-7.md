# Collect booking details and share only approved job information — verification 7

**Verdict: FAIL**

Verified on 2026-09-05 against implementation candidate
`0ad29bd64650c88b3f80e6f1e02b94d1d29c86c5` and live URL
`https://booking-intake-vault.sociobot.in`.

- Findings: **1 high**, 0 medium, 0 low.
- Untested declared claims: **0**.
- Documentation SHA: `02450e0e403b80c2852afe3b2829fa11e6b74afe`.
- Live `/health` reports that documentation SHA. The live entry JS and CSS
  SHA-256 hashes exactly match a fresh build of candidate `0ad29bd`; the later
  commits change documentation/copy files, not product runtime source.

## First screen in fresh browsers

Before scrolling, fresh 1440px desktop and 390px phone sessions both stated:

- Job: **Collect once. Share only job details.**
- Audience: field-service teams that keep client context private when assigning
  work.
- First action: **Try it with sample data**.

The action was visible and opened `/demo` in one click.

## Finding

### High — the advertised Route pass cannot be bought

The product advertises an optional one-time US$29 Route pass on the landing
page, terms, README, and manager Route pass screen. The manager screen’s
**Buy Route pass securely** control uses this checkout URL:

```text
https://api.sociobot.in/api/v1/products/booking-intake-vault/checkout
```

On 2026-09-05, an independent live `GET` returned **HTTP 404**. This is a
broken public purchase path and contradicts the route/page-link contract. The
product correctly keeps core booking tools free and enforces paid limits, but
the advertised paid offer is unavailable until the billing operator registers
the offer. This is the known external dependency, but it remains a finding and
prevents a PASS.

## Clean-checkout claims and quality gates

I made a new clone at candidate `0ad29bd`, confirmed that `frontend/dist` was
absent, ran `npm ci`, and then ran every manifest command exactly as written.
The first command, `@claim:zero-config`, built the missing web artifact itself.
All 17 commands passed:

- `zero-config`, `demo-isolation`, `server-field-redaction`,
  `automatic-deletion`, `worker-link-expiry`, `csv-export`, `no-trackers`,
  `offline-reload`, `entra-manager`, `paid-boundaries`, `rate-limits`,
  `response-policy`, `route-metadata`, `build-identity`, `mobile-keyboard`,
  `token-hashing`, and `durable-snapshot`.

Aggregate results from that same clone:

```text
npm test                         PASS — 3 Vitest, 19 Rust, 20 Playwright
npm run check                    PASS — 0 errors, 0 warnings
cargo fmt --all -- --check       PASS
cargo clippy ... -D warnings     PASS
npm run build                    PASS
npm audit --audit-level=high     PASS — 0 vulnerabilities
git diff --check                 PASS
```

This proves the verification-6 clean-clone failure is fixed. The browser claim
logs are in the disposable clean clone’s `claim-logs/` directory; the live
evidence is under `/work/.evidence/`.

## Live product checks

- Demo: `/demo` showed the persistent **Demo — sample data, nothing is
  saved** label, Nadia Patel’s realistic eight-answer manager record, and
  Morgan Lee’s five-fact worker ticket. The worker portion omitted Nadia’s name
  and `Warranty account NW-204`. **Reset demo** kept the banner and created a
  new sample workspace. The live public form digest before and after the demo
  check was identical:
  `bc4eb399fbbd0c6c6f08948b24e9bd62dfa7519251656d9dfe3ef1c23101f378`.
- Privacy and offline: fresh landing/demo traffic used only the product origin.
  After service-worker control, an offline reload of `/demo` retained the
  sample with no console errors. The active cache was
  `private-intake-shell-v5` and no waiting update existed.
- Accessibility: `verify-url.sh` passed `/`, `/demo`, `/book`, `/admin`,
  `/privacy`, and `/terms` with title, language, main landmark, image-alt,
  and console checks clean. Playwright axe checks found no serious or critical
  violations on those routes, an invalid worker route, or the designed 404. At
  390px there was no horizontal overflow; reduced motion matched; the first Tab
  focused the 48.8px skip link with a solid outline. The standalone axe CLI was
  attempted but its Selenium Chrome session exited in this container, so the
  successful Playwright axe integration was used instead.
- Routes and metadata: public application routes returned 200 with their own
  titles and canonicals. The unknown route returned the expected designed HTTP
  404; its browser console 404 was treated as expected, not a defect. Robots,
  sitemap, favicon, and touch icon returned 200.
- Backend: `/health` returned status `ok`; malformed public input returned
  422; a 70KB request returned 413 while preserving CSP, HSTS, nosniff, and
  no-store. Twenty session checks from one first forwarded address returned 200
  and request 21 returned 429 with `Retry-After: 58`; a different first
  address returned 200. Tenant isolation, redaction, token hashing, snapshot
  restoration, and restart-safe migrations are covered by the passing Rust and
  Playwright claim suite without touching production tenants.
- Earlier findings: the deep-link, contrast, configuration, validation,
  target-size, favicon, duplicate-ID, LCP/CLS, offline-console, forwarded-IP,
  Entra, claim-manifest, demo, CSV, metadata, and 413-header findings recorded
  in verification reports 1–6 are covered by the current passing tests and
  live checks above. Verification 6’s missing web-artifact command is directly
  retested from the unbuilt clone.

## Result

The free booking-intake job works end to end and every declared claim was
tested successfully. **FAIL** remains required because the public US$29 Route
pass checkout is a verified HTTP 404. Register the prepared billing offer, then
retest the checkout response and the complete paid purchase/verification path
before declaring PASS.
