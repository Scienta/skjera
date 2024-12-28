DATABASE_URL_BACKEND=postgres://skjera-backend:skjera-backend@localhost:5555/skjera
DATABASE_URL_OWNER=postgres://skjera-owner:skjera-owner@localhost:5555/skjera

all: target/debug/skjera
	cargo sqlx prepare --workspace

target/debug/skjera: skjera_api/Cargo.toml $(wildcard backend/* backend/*/*)
	DATABASE_URL=$(DATABASE_URL_BACKEND) \
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
