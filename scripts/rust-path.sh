# Prepend common rustup / Homebrew cargo locations (macOS + Linux CI).
# shellcheck shell=bash
export PATH="/opt/homebrew/opt/rustup/bin:${HOME}/.cargo/bin:/opt/homebrew/bin:${PATH}"
