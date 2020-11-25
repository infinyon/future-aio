RUST_DOCKER_IMAGE=rust:latest

build-all:
	cargo build --all-features

test-all:	test-tls-all
	cargo test --all-features

test-tls-all:	test_rustls test_native_tls_pk12 test_native_tls_x509

test_rustls:
	cargo test --features rust_tls

test_native_tls_pk12:
	cargo test --features native2_tls test_native_tls_pk12

test_native_tls_x509:
	cargo test --features native2_tls test_native_tls_x509

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

check-clippy:	install-clippy
	cargo clippy --all-targets --all-features -- -D warnings


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