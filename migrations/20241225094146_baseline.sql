CREATE SCHEMA skjera;

GRANT ALL ON SCHEMA skjera TO "skjera-backend";

CREATE TABLE skjera.employee
(
    id   SERIAL8,
    name VARCHAR NOT NULL CHECK ( LENGTH(TRIM(name)) > 0 ),

    PRIMARY KEY (id)
);

GRANT ALL ON skjera.employee TO "skjera-backend";

INSERT INTO skjera.employee(name)
VALUES ('Trygve Laugstøl');
