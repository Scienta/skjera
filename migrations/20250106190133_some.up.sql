ALTER TABLE some_account
    ADD COLUMN authenticated BOOL NOT NULL DEFAULT FALSE;

ALTER TABLE some_account
    ALTER COLUMN authenticated DROP DEFAULT;
