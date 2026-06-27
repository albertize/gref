#!/bin/sh
set -eu

REPO="${GREF_REPO:-albertize/gref}"
VERSION="latest"
PREFIX=""
BIN_DIR=""
VIM_DIR=""
INSTALL_VIM=1
VIM_PACK=0

usage() {
  cat <<'EOF'
Install gref.

Usage:
  install.sh [options]

Options:
  --version VERSION   Install a release tag such as v2.2.0 (default: latest)
  --prefix PATH       Install under PATH/bin and PATH/share/vim/vimfiles
  --bin-dir PATH      Install gref into PATH
  --vim-dir PATH      Install Vim runtime into PATH
  --vim-pack          Install Vim runtime as a native Vim package
  --no-vim            Install only the gref binary
  -h, --help          Show this help

Environment:
  GREF_REPO           GitHub repo to download from (default: albertize/gref)
EOF
}

die() {
  echo "gref install: $*" >&2
  exit 1
}

have() {
  command -v "$1" >/dev/null 2>&1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      [ "$#" -ge 2 ] || die "--version requires a value"
      VERSION="$2"
      shift 2
      ;;
    --prefix)
      [ "$#" -ge 2 ] || die "--prefix requires a value"
      PREFIX="${2%/}"
      shift 2
      ;;
    --bin-dir)
      [ "$#" -ge 2 ] || die "--bin-dir requires a value"
      BIN_DIR="${2%/}"
      shift 2
      ;;
    --vim-dir)
      [ "$#" -ge 2 ] || die "--vim-dir requires a value"
      VIM_DIR="${2%/}"
      shift 2
      ;;
    --vim-pack)
      VIM_PACK=1
      shift
      ;;
    --no-vim)
      INSTALL_VIM=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
done

detect_asset() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux) os_name="linux" ;;
    Darwin) os_name="darwin" ;;
    *) die "unsupported OS: $os" ;;
  esac

  case "$arch" in
    x86_64|amd64) arch_name="amd64" ;;
    aarch64|arm64) arch_name="arm64" ;;
    *) die "unsupported architecture: $arch" ;;
  esac

  printf 'gref-%s-%s.tar.gz\n' "$os_name" "$arch_name"
}

download() {
  url="$1"
  out="$2"
  if have curl; then
    curl -fsSL "$url" -o "$out"
  elif have wget; then
    wget -q "$url" -O "$out"
  else
    die "curl or wget is required"
  fi
}

verify_checksum() {
  asset="$1"
  sums="$2"
  awk -v asset="$asset" '$2 == asset || $2 == "*" asset { print; found = 1 } END { exit found ? 0 : 1 }' "$sums" > "$sums.asset" || die "checksum for $asset not found"
  if have sha256sum; then
    sha256sum -c "$sums.asset" >/dev/null
  elif have shasum; then
    shasum -a 256 -c "$sums.asset" >/dev/null
  else
    die "sha256sum or shasum is required"
  fi
}

install_file() {
  src="$1"
  dst="$2"
  mode="$3"
  mkdir -p "$(dirname "$dst")"
  cp "$src" "$dst"
  chmod "$mode" "$dst"
}

default_bin_dir() {
  if [ -n "$BIN_DIR" ]; then
    printf '%s\n' "$BIN_DIR"
  elif [ -n "$PREFIX" ]; then
    printf '%s/bin\n' "$PREFIX"
  else
    printf '%s/.local/bin\n' "$HOME"
  fi
}

default_vim_dir() {
  if [ -n "$VIM_DIR" ]; then
    printf '%s\n' "$VIM_DIR"
  elif [ -n "$PREFIX" ]; then
    printf '%s/share/vim/vimfiles\n' "$PREFIX"
  elif [ "$VIM_PACK" -eq 1 ]; then
    printf '%s/.vim/pack/gref/start/gref\n' "$HOME"
  else
    printf '%s/.vim\n' "$HOME"
  fi
}

install_from_package() {
  package_dir="$1"
  bin_dir="$(default_bin_dir)"
  vim_dir="$(default_vim_dir)"

  [ -f "$package_dir/bin/gref" ] || die "package is missing bin/gref"
  install_file "$package_dir/bin/gref" "$bin_dir/gref" 755
  echo "installed gref to $bin_dir/gref"

  if [ "$INSTALL_VIM" -eq 1 ]; then
    [ -f "$package_dir/vim/plugin/gref.vim" ] || die "package is missing vim/plugin/gref.vim"
    [ -f "$package_dir/vim/autoload/gref.vim" ] || die "package is missing vim/autoload/gref.vim"
    install_file "$package_dir/vim/plugin/gref.vim" "$vim_dir/plugin/gref.vim" 644
    install_file "$package_dir/vim/autoload/gref.vim" "$vim_dir/autoload/gref.vim" 644
    echo "installed Vim runtime to $vim_dir"
  fi

  case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) echo "note: $bin_dir is not in PATH" ;;
  esac
}

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd || pwd)"
if [ -f "$script_dir/bin/gref" ]; then
  install_from_package "$script_dir"
  exit 0
fi

have tar || die "tar is required"
have mktemp || die "mktemp is required"

asset="$(detect_asset)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

if [ "$VERSION" = "latest" ]; then
  base_url="https://github.com/$REPO/releases/latest/download"
else
  base_url="https://github.com/$REPO/releases/download/$VERSION"
fi

echo "downloading $asset from $REPO ($VERSION)"
download "$base_url/$asset" "$tmp_dir/$asset"
download "$base_url/SHA256SUMS" "$tmp_dir/SHA256SUMS"

(cd "$tmp_dir" && verify_checksum "$asset" "SHA256SUMS")

mkdir "$tmp_dir/package"
tar xzf "$tmp_dir/$asset" -C "$tmp_dir/package"
package_root="$(find "$tmp_dir/package" -mindepth 1 -maxdepth 1 -type d | head -n 1)"
[ -n "$package_root" ] || die "archive did not contain a package directory"

install_from_package "$package_root"
