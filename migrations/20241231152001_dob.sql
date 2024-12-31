ALTER TABLE skjera.employee
    DROP COLUMN IF EXISTS dob_month,
    DROP COLUMN IF EXISTS dob_day,
    DROP COLUMN IF EXISTS dob;

ALTER TABLE skjera.employee
    ADD COLUMN dob DATE;

UPDATE skjera.employee
SET dob = '1980-12-09'
WHERE email = 'trygvis@scienta.no';
