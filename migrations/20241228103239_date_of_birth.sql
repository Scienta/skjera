ALTER TABLE skjera.employee
    DROP COLUMN IF EXISTS dob_month,
    DROP COLUMN IF EXISTS dob_day;

ALTER TABLE skjera.employee
    ADD COLUMN dob_month INT CHECK ( dob_month >= 1 AND dob_month <= 12 ),
    ADD COLUMN dob_day   INT CHECK ( dob_day >= 1 AND dob_day <= 31 );

UPDATE skjera.employee
SET dob_month=12,
    dob_day=9
WHERE email = 'trygvis@scienta.no';
