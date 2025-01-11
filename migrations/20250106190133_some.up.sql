ALTER TABLE skjera.some_account
    ADD COLUMN authenticated BOOL NOT NULL DEFAULT FALSE;

ALTER TABLE skjera.some_account
    ALTER COLUMN authenticated DROP DEFAULT;
