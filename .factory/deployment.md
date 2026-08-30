# Container deployment contract

Private Intake is a stateful, single-tenant SQLite service. The image starts and
serves a usable default booking form with only `PORT`; manager ownership is
claimed by the first valid Sociobot Entra identity.

For revision-to-revision data durability, the Container App must also keep:

- One replica (`minReplicas: 1`, `maxReplicas: 1`)
- `DATABASE_URL=sqlite:///tmp/booking-intake-vault.db?mode=rwc`
- `DATABASE_BACKUP_PATH=/data/booking-intake-vault-local.db`
- Environment storage `booking-intake-vault-data`
- Volume `vault-data` mounted at `/data`

The application copies the durable snapshot to local SQLite at startup and
copies the committed database back after successful API requests. If a generic
deployment removes the mount, the app still accepts bookings, but data cannot
survive replacement of that revision. The repair deployment therefore reapplies
the mount and one-replica setting after publishing the new image.

No manager password or application secret is required. Entra tenant and client
settings have checked-in public defaults and may be overridden with environment
variables. The container logs whether database and workspace settings use
defaults or overrides without printing identity tokens or booking values.
