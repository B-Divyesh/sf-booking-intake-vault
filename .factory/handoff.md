# Private Intake — independent verification handoff

## Result: FAIL

Candidate `827582799f19138a0e7c542db355f71f4ae8ea1a` was independently tested
on 2026-08-28 from a clean checkout and against
`https://booking-intake-vault.sociobot.in`.

The rollout itself is healthy: live `/health` reports the exact candidate,
supported deep links return 200, and all deployed static artifacts match the
candidate build by SHA-256. Release is still blocked by:

1. Serious axe contrast failures in the worker brief and multiple manager
   states (measured ratios 1.46:1–2.48:1).
2. Paid 9–12-field and 7–14-day-link capabilities can be enabled through the
   backend API without any license.
3. Production remains unconfigured: the public form returns 404 and the
   unauthenticated one-time setup endpoint leaves the sole vault claimable by
   the first visitor.
4. The API persists malformed date values, several mobile links are under the
   required 44 px target size, a missing favicon logs a normal-load 404, and
   duplicate form IDs cause 500 instead of validation feedback.

Full evidence and exact reproduction results are in
`.factory/verification-2.md`.

## Verification summary

Passed from a detached clean worktree:

```sh
npm ci
npm test
npm run build
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
npm audit --audit-level=high
git diff --check
```

The repository's 13 tests passed, as did the exact production build. Manual
coverage included setup/login/form boundaries, full booking/admin/worker CRUD,
server-side redaction and immutable snapshots, export/deletion/token rotation,
restart persistence, 20 concurrent writes, 300 concurrent health reads,
desktop/mobile/keyboard/reduced-motion, expanded axe state coverage, service
worker update/offline reload, outbound-request/privacy review, TLS/CORS/security
headers/caching, artifact hashes, and Lighthouse.

Docker is unavailable in this verifier container; the native release binary
was tested with only `PORT` and with the candidate build identity. No product
code was changed. Production was not claimed or populated.

## Next steps

Correct the blockers above, add axe coverage for every task state and backend
entitlement tests, configure the production owner safely, then repeat clean
local and configured-live end-to-end verification.
