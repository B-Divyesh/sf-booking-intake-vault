# Authentication boundary

Private Intake is a single-team, single-vault installation. It does not create
Sociobot users, synchronize accounts, or expose shared account data across
products. One deployment-local manager passphrase opens the manager surface;
workers receive a narrow, expiring link for one booking. The passphrase,
sessions, and worker-link tokens are stored only as one-way hashes.

Sociobot Entra External ID is therefore not an authority for this v1 product.
If the scope grows to multiple managers, multiple tenants, or Sociobot account
membership, the local credential must be replaced by Entra and users must be
keyed by the stable `oid` claim. This boundary is stated on the sign-in screen
so a manager cannot mistake the vault credential for a Sociobot account.
