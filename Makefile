RUST_DOCKER_IMAGE=rust:latest
CERT_OPTS ?=
PFX_OPTS ?= ""

build-all:
	cargo build --all-features

.PHONY: certs
certs:
	make -C certs generate-certs PFX_OPTS=${PFX_OPTS}

cert-patch-macos:
	sed -i '' 's/RSA PRIVATE KEY/PRIVATE KEY/' certs/test-certs/server-hs.key

.PHONY: test-all run-test-all
test-all: certs test-derive setup-http-server run-test-all
run-test-all:
	TEST_PORT=$$(cat tmp-PORT) cargo test --all-features
	$(MAKE) teardown-http-server

.PHONY: test-http run-test-http
test-http: certs setup-http-server run-test-http
run-test-http:
	TEST_PORT=$$(cat tmp-PORT) cargo test --all-features test_http_client
	$(MAKE) teardown-http-server

install-wasm-pack:
	which wasm-pack || curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

test-wasm: install-wasm32 install-wasm-pack
	wasm-pack test --firefox --headless
	wasm-pack test --chrome --headless

test-wasm-safari: install-wasm32 install-wasm-pack
	wasm-pack test --safari --headless

test-derive:
	cd async-test-derive; cargo test

check-wasm: install-wasm32
	cargo build --target wasm32-unknown-unknown --all-features

install_windows_on_mac:
	rustup target add x86_64-pc-windows-gnu
	brew install mingw-w64

install_linux:
	rustup target add x86_64-unknown-linux-musl

# build linux version
build_linux:	install_linux
	cargo build --target ${TARGET_LINUX}

# build windows version
build-windows:
	cargo build --target=x86_64-pc-windows-gnu

install-fmt:
	rustup component add rustfmt

check-fmt:	install-fmt
	cargo fmt -- --check

install-clippy:
	rustup component add clippy

install-wasm32:
	rustup target add wasm32-unknown-unknown

setup-http-server: certs $(CERT_OPTS)
	cargo binstall -y http-server
	cargo binstall -y portpicker-cli
	portpicker > tmp-PORT
	echo Picked port $$(cat tmp-PORT)
	http-server --tls \
		--tls-key certs/test-certs/server-hs.key \
		--tls-cert certs/test-certs/server.crt \
		--tls-key-algorithm pkcs8 -p $$(cat tmp-PORT) &

teardown-http-server:
	killall http-server
	rm -f tmp-PORT

check-clippy:	install-clippy install-wasm32
	cargo clippy --all-features
	cargo check --target wasm32-unknown-unknown --all-features


cargo_cache_dir:
	mkdir -p .docker-cargo

docker_linux_test:	cargo_cache_dir
	 docker run --rm --volume ${PWD}:/src --workdir /src  \
	 	-e USER -e CARGO_HOME=/src/.docker-cargo \
		-e CARGO_TARGET_DIR=/src/target-docker \
	  	${RUST_DOCKER_IMAGE} cargo test

docker_linux_test_large:	cargo_cache_dir
	 docker run --rm --volume ${PWD}:/src --workdir /src  \
	 	-e USER -e CARGO_HOME=/src/.docker-cargo \
		-e CARGO_TARGET_DIR=/src/target-docker \
	 	--env RUST_LOG=trace \
	  	${RUST_DOCKER_IMAGE} cargo test zero_copy_large_size
