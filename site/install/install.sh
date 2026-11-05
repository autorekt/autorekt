#!/bin/sh
# Installs autorekt on Linux and macOS: curl -fsSL {{SITE}}/install.sh | sh
set -eu

base="{{SITE}}/dl"
case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=darwin ;;
  *) echo "autorekt: no build for $(uname -s); on Windows run: irm {{SITE}}/install.ps1 | iex" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=x64 ;;
  arm64 | aarch64) arch=arm64 ;;
  *) echo "autorekt: no build for $(uname -m)" >&2; exit 1 ;;
esac
# A shell running under Rosetta reports x86_64 on Apple silicon.
if [ "$os" = darwin ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null)" = 1 ]; then arch=arm64; fi
file="autorekt-$os-$arch"

if command -v curl >/dev/null 2>&1; then fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then fetch() { wget -qO "$2" "$1"; }
else echo "autorekt: install needs curl or wget" >&2; exit 1; fi
if command -v sha256sum >/dev/null 2>&1; then sha() { sha256sum "$1"; }
else sha() { shasum -a 256 "$1"; }; fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "downloading $base/$file"
fetch "$base/$file" "$tmp/$file"
fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS"
want=$(grep " $file\$" "$tmp/SHA256SUMS" | cut -d' ' -f1)
got=$(sha "$tmp/$file" | cut -d' ' -f1)
if [ -z "$want" ] || [ "$want" != "$got" ]; then echo "autorekt: checksum mismatch for $file" >&2; exit 1; fi

dir="$HOME/.local/bin"
mkdir -p "$dir"
chmod +x "$tmp/$file"
mv "$tmp/$file" "$dir/autorekt"
echo "installed $("$dir/autorekt" --version) to $dir/autorekt"

case ":$PATH:" in
  *":$dir:"*) ;;
  *)
    case "${SHELL:-}" in
      */zsh) rc="$HOME/.zshrc" ;;
      */bash) rc="$HOME/.bashrc" ;;
      *) rc="$HOME/.profile" ;;
    esac
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$rc"
    echo "added $dir to PATH in $rc; open a new terminal, or run: export PATH=\"$dir:\$PATH\""
    ;;
esac
echo "next: autorekt show"
