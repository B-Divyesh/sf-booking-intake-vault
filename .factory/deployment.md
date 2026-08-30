# Container deployment contract

Private Intake is a stateful, single-tenant SQLite service. Every Container
App revision must keep these settings in addition to the work order's normal
image, ingress, custom-domain, and build-SHA settings:

- One replica (`minReplicas: 1`, `maxReplicas: 1`).
- Environment variable `DATABASE_URL=sqlite:///tmp/booking-intake-vault.db?mode=rwc`.
- Environment variable `DATABASE_BACKUP_PATH=/data/booking-intake-vault-local.db`.
- Azure Files environment storage `booking-intake-vault-data`, mounted as the
  `vault-data` volume at `/data`.
- A platform secret exposed as `MANAGER_PASSPHRASE`, so the deployment owner
  can recover manager access after a revision change without changing source.

The application restores the local SQLite working copy from that durable file
at startup and snapshots it after every successful API request. A generic
container deployment that replaces the template with only `PORT` removes the
mount and makes the next revision appear unconfigured. Reapply this stateful
template before directing traffic to a new revision.

For a genuinely empty storage volume, the same secret can also be exposed as
`INITIAL_ADMIN_PASSPHRASE` for the first boot. Never put either value in
source, command output, or deployment logs. `MANAGER_PASSPHRASE` may remain as
the recoverable owner credential; the application stores only its Argon2 hash.
