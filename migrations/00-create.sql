-- This is a psql script, not a generic sql script

CREATE DATABASE skjera;

\c skjera

CREATE ROLE "skjera-backend" WITH LOGIN ENCRYPTED PASSWORD 'skjera-backend';

CREATE ROLE "skjera-owner" WITH LOGIN ENCRYPTED PASSWORD 'skjera-owner';
GRANT ALL ON DATABASE skjera TO "skjera-owner";

GRANT ALL ON SCHEMA public TO "skjera-owner";
GRANT USAGE ON SCHEMA public TO "skjera-backend";
