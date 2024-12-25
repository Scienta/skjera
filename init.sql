\c postgres

SELECT PG_TERMINATE_BACKEND(pid)
FROM pg_stat_activity
WHERE datname = 'skjera';

DROP DATABASE IF EXISTS skjera;
DROP ROLE IF EXISTS "skjera-backend";
DROP ROLE IF EXISTS "skjera-owner";

---

CREATE DATABASE skjera;

CREATE ROLE "skjera-backend" WITH LOGIN ENCRYPTED PASSWORD 'skjera-backend';
-- GRANT ALL ON DATABASE skjera to "skjera-backend";

CREATE ROLE "skjera-owner" WITH LOGIN ENCRYPTED PASSWORD 'skjera-owner';
GRANT ALL ON DATABASE skjera TO "skjera-owner";

\c skjera

GRANT ALL ON SCHEMA public TO "skjera-owner";
GRANT USAGE ON SCHEMA public TO "skjera-backend";
