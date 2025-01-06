DATABASE_URL=postgres://skjera:skjera@localhost:5555/skjera

export DATABASE_URL

all: target/debug/skjera
	cargo sqlx prepare --workspace

target/debug/skjera: skjera_api/Cargo.toml $(wildcard backend/* backend/*/*)
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
