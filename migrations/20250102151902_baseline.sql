DROP TABLE IF EXISTS skjera.some_account;
DROP TABLE IF EXISTS skjera.employee;
DROP SEQUENCE IF EXISTS skjera.id_seq;

CREATE SEQUENCE id_seq START 1000000;

CREATE TABLE skjera.employee
(
    id    BIGINT DEFAULT NEXTVAL('id_seq'),
    name  VARCHAR NOT NULL CHECK (TRIM(name) = name AND LENGTH(TRIM(name)) > 0 ),
    email VARCHAR NOT NULL CHECK (TRIM(email) = email AND LENGTH(TRIM(email)) > 0 ),
    dob   DATE,

    PRIMARY KEY (id)
);

CREATE TABLE skjera.some_account
(
    id       BIGINT DEFAULT NEXTVAL('id_seq'),
    employee BIGINT  NOT NULL REFERENCES skjera.employee,
    network  VARCHAR NOT NULL CHECK (TRIM(network) = network AND LENGTH(network) > 0 ),
    nick     VARCHAR NOT NULL CHECK (TRIM(nick) = nick AND LENGTH(nick) > 0 ),
    url      VARCHAR NOT NULL CHECK (TRIM(url) = url AND LENGTH(url) > 0 ),

    PRIMARY KEY (id)
);

INSERT INTO skjera.employee(email, name, dob)
VALUES ('trygvis@scienta.no', 'Trygve Laugstøl', '1980-12-09'),
       ('hege.storvold@scienta.no', 'Hege Størvold', NULL);

INSERT INTO skjera.some_account(employee, network, nick, url)
VALUES ((SELECT id FROM skjera.employee WHERE name = 'Trygve Laugstøl'),
        'github',
        'trygvis',
        'https://github.com/trygvis'),
       ((SELECT id FROM skjera.employee WHERE name = 'Trygve Laugstøl'),
        'linkedin',
        'trygvis',
        'https://www.linkedin.com/in/trygvis/'),
       ((SELECT id FROM skjera.employee WHERE name = 'Hege Størvold'),
        'github',
        'hegepege',
        'https://github.com/hegepege')
;
