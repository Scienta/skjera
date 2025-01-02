-- This is a psql script, not a generic sql script

\c postgres

SELECT PG_TERMINATE_BACKEND(pid)
FROM pg_stat_activity
WHERE datname = 'skjera';

DROP DATABASE IF EXISTS skjera;
DROP ROLE IF EXISTS skjera;
