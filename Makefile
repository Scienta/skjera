all: skjera_api/Cargo.toml

skjera_api/Cargo.toml: skjera-api.yaml
	rm -rf skjera_api
	bin/openapi-generator-cli generate \
		-g rust-axum \
		-o skjera_api \
		-i skjera-api.yaml \
		--package-name skjera_api
