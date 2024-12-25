all: target/debug/skjera

target/debug/skjera: skjera_api/Cargo.toml
	cargo build

skjera_api/Cargo.toml: skjera-api.yaml
	bin/openapi-generator-cli generate \
		-g rust-axum \
		-o skjera_api \
		-i skjera-api.yaml \
		--package-name skjera_api

clean:
	rm -rf skjera_api
	rm -rf target
