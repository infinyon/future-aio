TARGET_LINUX=x86_64-unknown-linux-musl
TARGET_DARWIN=x86_64-apple-darwin
RUSTV = 1.43.1
RUST_DOCKER_IMAGE=rust:${RUSTV}


build-all:
	cargo build
	cargo build --features tls
	cargo build --features timer
	cargo build --features net
	cargo build --features fs
	cargo build --features zero_copy
	cargo build --features mmap
	cargo build --features task_unstable
	cargo build --features fixture

test-all:
	cargo test
	cargo test --features tls
	cargo test --features timer
	cargo test --features net
	cargo test --features fs
	cargo test --features zero_copy
	cargo test --features mmap
	cargo test --features task_unstable
	cargo test --features fixture 


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