-- This is a psql script, not a generic sql script

CREATE ROLE skjera WITH LOGIN ENCRYPTED PASSWORD 'skjera';

CREATE DATABASE skjera;

\c skjera

GRANT ALL ON DATABASE skjera TO skjera;

CREATE SCHEMA skjera;
GRANT ALL ON SCHEMA skjera TO skjera;
REVOKE ALL ON SCHEMA public FROM skjera;
