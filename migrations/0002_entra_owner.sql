ALTER TABLE workspaces ADD COLUMN owner_oid TEXT;
ALTER TABLE workspaces DROP COLUMN passphrase_hash;
DROP TABLE sessions;
