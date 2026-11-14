#!/usr/bin/env bash
# Agent host profile setup: installs the toolchains into $HOME, which the host keeps on its
# persist disk. Idempotent and fast when nothing changed; runs at `project add` and every boot.
set -euo pipefail
cd "$(dirname "$0")/../.."
arch=$(uname -m)
node_arch=${arch/aarch64/arm64}; node_arch=${node_arch/x86_64/x64}

if [ ! -x "$HOME/.cargo/bin/rustup" ]; then
  curl -fsSL https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain none
fi
export PATH="$HOME/.cargo/bin:$HOME/.dotnet:$HOME/.local/node/bin:$PATH"
rustup toolchain install  # rust-toolchain.toml: version, components, wasm32-wasip2

if ! "$HOME/.dotnet/dotnet" --list-sdks 2>/dev/null | grep -q '^10\.'; then
  curl -fsSL https://dot.net/v1/dotnet-install.sh | bash -s -- --channel 10.0 --install-dir "$HOME/.dotnet"
fi

if ! node --version 2>/dev/null | grep -q '^v24\.'; then
  tarball=$(curl -fsSL https://nodejs.org/dist/latest-v24.x/SHASUMS256.txt | awk -v a="linux-$node_arch.tar.xz" '$2 ~ a"$" {print $2}')
  rm -rf "$HOME/.local/node" && mkdir -p "$HOME/.local/node"
  curl -fsSL "https://nodejs.org/dist/latest-v24.x/$tarball" | tar -xJ -C "$HOME/.local/node" --strip-components 1
fi

nextest=linux; [ "$arch" = aarch64 ] && nextest=linux-arm
command -v cargo-nextest >/dev/null || curl -fsSL "https://get.nexte.st/latest/$nextest" | tar -xz -C "$HOME/.cargo/bin"
command -v cargo-sweep >/dev/null || cargo install --quiet --locked cargo-sweep

# shellcheck disable=SC2016  # expanded at login
profile_line='export PATH="$HOME/.cargo/bin:$HOME/.dotnet:$HOME/.local/node/bin:$PATH" DOTNET_ROOT="$HOME/.dotnet" DOTNET_CLI_TELEMETRY_OPTOUT=1'
grep -qxF "$profile_line" "$HOME/.profile" 2>/dev/null || echo "$profile_line" >>"$HOME/.profile"
echo "rust $(rustc --version | cut -d' ' -f2), dotnet $(dotnet --version), node $(node --version), $(cargo nextest --version | head -1)"
