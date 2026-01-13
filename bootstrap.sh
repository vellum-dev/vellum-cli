#!/bin/sh
set -e

APK_TOOLS_VERSION="__APK_TOOLS_VERSION__"
VELLUM_AARCH64_SHA256="__VELLUM_AARCH64_SHA256__"
VELLUM_ARMV7_SHA256="__VELLUM_ARMV7_SHA256__"
APK_AARCH64_SHA256="__APK_AARCH64_SHA256__"
APK_ARMV7_SHA256="__APK_ARMV7_SHA256__"
SIGNING_KEY_SHA256="__SIGNING_KEY_SHA256__"

VELLUM_ROOT="/home/root/.vellum"
OFFLINE_DIR=""
NO_VERIFY=false

usage() {
    echo "Usage: $0 [--offline <dir>] [--no-verify]"
    echo ""
    echo "Options:"
    echo "  --offline <dir>  Install from local files instead of downloading"
    echo "                   Directory should contain:"
    echo "                     - apk-aarch64 or apk-armv7"
    echo "                     - vellum-linux-arm64 or vellum-linux-armv7"
    echo "                     - packages.rsa.pub"
    echo "  --no-verify      Skip SHA256 checksum verification"
    exit 1
}

while [ $# -gt 0 ]; do
    case "$1" in
        --offline)
            [ -z "$2" ] && usage
            OFFLINE_DIR="$2"
            shift 2
            ;;
        --no-verify)
            NO_VERIFY=true
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
done

if [ -n "$OFFLINE_DIR" ] && [ ! -d "$OFFLINE_DIR" ]; then
    echo "Error: Offline directory does not exist: $OFFLINE_DIR"
    exit 1
fi

FRESH_INSTALL=false
if [ ! -d "$VELLUM_ROOT" ]; then
    FRESH_INSTALL=true
fi

cleanup() {
    if [ "$FRESH_INSTALL" = true ]; then
        echo "Installation failed, cleaning up..."
        rm -rf "$VELLUM_ROOT"
    fi
}
trap cleanup EXIT
VELLUM_CLI_RELEASES="https://github.com/vellum-dev/vellum-cli/releases/latest/download"
VELLUM_PACKAGES_REPO="https://raw.githubusercontent.com/vellum-dev/vellum/main"
VELLUM_APK_RELEASES="https://github.com/vellum-dev/apk-tools/releases/download/$APK_TOOLS_VERSION"

verify_sha256() {
    if [ "$NO_VERIFY" = true ]; then
        return 0
    fi
    file="$1"
    expected="$2"
    actual=$(sha256sum "$file" | cut -d' ' -f1)
    if [ "$actual" != "$expected" ]; then
        echo "SHA256 verification failed for $file"
        echo "Expected: $expected"
        echo "Got:      $actual"
        rm -f "$file"
        exit 1
    fi
}

echo "Installing vellum..."

ARCH=$(uname -m)
case "$ARCH" in
    aarch64) APK_ARCH="aarch64" ;;
    armv7l)  APK_ARCH="armv7" ;;
    *)       echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

mkdir -p "$VELLUM_ROOT"/{bin,etc/apk/keys,etc/apk/cache,lib/apk/db,share/bash-completion/completions,state,local-repo}

if [ -n "$OFFLINE_DIR" ]; then
    echo "Installing apk.vellum from offline directory..."
    cp "$OFFLINE_DIR/apk-$APK_ARCH" "$VELLUM_ROOT/bin/apk.vellum"
else
    echo "Downloading apk.vellum..."
    wget -q "$VELLUM_APK_RELEASES/apk-$APK_ARCH" -O "$VELLUM_ROOT/bin/apk.vellum"
fi
case "$APK_ARCH" in
    aarch64) verify_sha256 "$VELLUM_ROOT/bin/apk.vellum" "$APK_AARCH64_SHA256" ;;
    armv7)   verify_sha256 "$VELLUM_ROOT/bin/apk.vellum" "$APK_ARMV7_SHA256" ;;
esac
chmod +x "$VELLUM_ROOT/bin/apk.vellum"

if [ -n "$OFFLINE_DIR" ]; then
    echo "Installing vellum from offline directory..."
    case "$APK_ARCH" in
        aarch64) cp "$OFFLINE_DIR/vellum-linux-arm64" "$VELLUM_ROOT/bin/vellum" ;;
        armv7)   cp "$OFFLINE_DIR/vellum-linux-armv7" "$VELLUM_ROOT/bin/vellum" ;;
    esac
else
    echo "Downloading vellum..."
    case "$APK_ARCH" in
        aarch64) wget -q "$VELLUM_CLI_RELEASES/vellum-linux-arm64" -O "$VELLUM_ROOT/bin/vellum" ;;
        armv7)   wget -q "$VELLUM_CLI_RELEASES/vellum-linux-armv7" -O "$VELLUM_ROOT/bin/vellum" ;;
    esac
fi
case "$APK_ARCH" in
    aarch64) verify_sha256 "$VELLUM_ROOT/bin/vellum" "$VELLUM_AARCH64_SHA256" ;;
    armv7)   verify_sha256 "$VELLUM_ROOT/bin/vellum" "$VELLUM_ARMV7_SHA256" ;;
esac
chmod +x "$VELLUM_ROOT/bin/vellum"

if [ -n "$OFFLINE_DIR" ]; then
    echo "Installing signing key from offline directory..."
    cp "$OFFLINE_DIR/packages.rsa.pub" "$VELLUM_ROOT/etc/apk/keys/packages.rsa.pub"
else
    echo "Downloading signing key..."
    wget -q "$VELLUM_PACKAGES_REPO/keys/packages.rsa.pub" -O "$VELLUM_ROOT/etc/apk/keys/packages.rsa.pub"
fi
verify_sha256 "$VELLUM_ROOT/etc/apk/keys/packages.rsa.pub" "$SIGNING_KEY_SHA256"

echo "Generating local signing key..."
if [ ! -f "$VELLUM_ROOT/etc/apk/keys/local.rsa" ]; then
    openssl genrsa -out "$VELLUM_ROOT/etc/apk/keys/local.rsa" 2048 2>/dev/null
    openssl rsa -in "$VELLUM_ROOT/etc/apk/keys/local.rsa" -pubout -out "$VELLUM_ROOT/etc/apk/keys/local.rsa.pub" 2>/dev/null
fi

echo "Configuring repositories..."
cat > "$VELLUM_ROOT/etc/apk/repositories" <<EOF
/home/root/.vellum/local-repo
https://packages.vellum.delivery
EOF

echo "Initializing local repository..."
mkdir -p "$VELLUM_ROOT/local-repo/$APK_ARCH"
LOCAL_KEY="$VELLUM_ROOT/etc/apk/keys/local.rsa"
(
    cd "$VELLUM_ROOT/local-repo/$APK_ARCH"
    touch APKINDEX
    tar -czf unsigned.tar.gz APKINDEX
    openssl dgst -sha1 -sign "$LOCAL_KEY" -out ".SIGN.RSA.local.rsa.pub" unsigned.tar.gz
    tar -cf sig.tar .SIGN.RSA.local.rsa.pub
    SIG_SIZE=$(stat -c %s ".SIGN.RSA.local.rsa.pub" 2>/dev/null || stat -f %z ".SIGN.RSA.local.rsa.pub")
    CONTENT_BLOCKS=$(( (512 + SIG_SIZE + 511) / 512 ))
    dd if=sig.tar bs=512 count=$CONTENT_BLOCKS 2>/dev/null | gzip -n -9 > sig.tar.gz
    cat sig.tar.gz unsigned.tar.gz > APKINDEX.tar.gz
    rm -f APKINDEX unsigned.tar.gz sig.tar sig.tar.gz .SIGN.RSA.local.rsa.pub
)

echo "Initializing apk database..."
"$VELLUM_ROOT/bin/apk.vellum" \
    --root "$VELLUM_ROOT" \
    --install-root / \
    --no-logfile \
    add --initdb

echo "Registering vellum package..."
"$VELLUM_ROOT/bin/apk.vellum" \
    --root "$VELLUM_ROOT" \
    --install-root / \
    --no-logfile \
    add vellum 2>/dev/null || true

if [ -n "$OFFLINE_DIR" ]; then
    echo "Skipping package index update and package installs (offline mode)"
else
    echo "Updating package index..."
    "$VELLUM_ROOT/bin/vellum" update

    echo "Installing mount-utils..."
    "$VELLUM_ROOT/bin/vellum" add mount-utils

    echo "Installing bash completion..."
    "$VELLUM_ROOT/bin/vellum" add vellum-bash-completion
fi

BASHRC="/home/root/.bashrc"
PATH_LINE="export PATH=\"$VELLUM_ROOT/bin:\$PATH\""
COMPLETION_LINE="[ -f \"$VELLUM_ROOT/share/bash-completion/completions/vellum\" ] && . \"$VELLUM_ROOT/share/bash-completion/completions/vellum\""

if [ -f "$BASHRC" ] && grep -qF ".vellum/bin" "$BASHRC"; then
    echo "PATH already configured in $BASHRC"
else
    echo "" >> "$BASHRC"
    echo "$PATH_LINE" >> "$BASHRC"
    echo "$COMPLETION_LINE" >> "$BASHRC"
    echo "Added vellum to PATH and completions in $BASHRC"
fi

trap - EXIT

echo ""
echo "Vellum installed successfully!"
echo "Run 'source ~/.bashrc' or start a new shell to use vellum."
if [ -n "$OFFLINE_DIR" ]; then
    echo ""
    echo "Offline install complete. When network is available, run:"
    echo "  vellum update"
    echo "  vellum add mount-utils"
    echo "  vellum add vellum-bash-completion"
fi
