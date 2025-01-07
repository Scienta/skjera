ALTER TABLE skjera.some_account
    ADD CONSTRAINT uq_some_account_network UNIQUE (network, network_instance)
