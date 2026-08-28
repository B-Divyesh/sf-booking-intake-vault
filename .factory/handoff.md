# Private Intake — verification handoff

## Result: FAIL

Candidate `84adef21c835e59314f6060be5947d6cc16a9107` was independently tested
on 2026-08-28 at `https://booking-intake-vault.sociobot.in`. Full evidence is
in `.factory/verification-3.md`.

The code and candidate assets build and test cleanly, and production serves
that exact build SHA. The deployed vault is, however, unconfigured:

```text
GET /api/session      -> configured:false, setup_allowed:false
GET /api/form/public  -> available:false
POST /api/setup       -> 401
```

Therefore `/book` cannot collect a booking and the product does not meet its
core hosted-intake/least-privilege worker-brief job. This is a deployment
release blocker, not an invitation to open public setup. Initialize it only
with the platform-controlled bootstrap secret on durable `/data` storage, then
retest configured production end to end.

Additional findings: mobile Lighthouse LCP was 2,624.915 ms against the
2.5-second target, and the intentional offline form fetch emits disconnected
network console errors. Docker could not be built in this container because the
Docker CLI is absent.

Local verification commands:

```sh
npm ci
npm run check
npm test
npm run build
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Do not release until the live initialization/persistence failure is resolved
and the verification report’s retest requirements pass.
