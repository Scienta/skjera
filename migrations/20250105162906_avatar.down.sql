ALTER TABLE skjera.some_account
    DROP COLUMN IF EXISTS network_instance,
    DROP COLUMN IF EXISTS subject,
    DROP COLUMN IF EXISTS name,
    DROP COLUMN IF EXISTS avatar;
