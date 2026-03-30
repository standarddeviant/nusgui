cargo install cargo-auditable cargo-zigbuild
cargo auditable zigbuild --release

# TODO: make prompt to copy binary to ~/.cargo/bin
cp ./target/release/nusgui ~/.cargo/bin
