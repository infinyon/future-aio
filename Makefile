install_windows_on_mac:
	rustup target add x86_64-pc-windows-gnu
	brew install mingw-w64


build-windows:
	cargo build --target=x86_64-pc-windows-gnu

test:
	cargo test

test_tokio:
	cargo test   --features=tokio2 --no-default-features 


test_asyncstd:
	cargo test --features=asyncstd --no-default-features