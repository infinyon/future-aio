RUST_DOCKER_IMAGE=rust:latest

build-all:
	cargo build --all-features

certs:
	make -C certs generate-certs

test-all:	certs test-derive
	cargo test --all-features

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

check-clippy:	install-clippy install-wasm32
	cargo clippy --all-targets --all-features -- -D warnings
	cargo clippy --target wasm32-unknown-unknown --all-targets --all-features -- -D warnings


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
