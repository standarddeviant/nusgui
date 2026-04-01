#!/usr/bin/env bash
set -euo pipefail

# install tools
cargo install cargo-auditable cargo-zigbuild

# perform build
cargo auditable zigbuild --release

# copy build artifact
# TODO: make prompt to copy binary to ~/.cargo/bin
cp ./target/release/nusgui ~/.cargo/bin
