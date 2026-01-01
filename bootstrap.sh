#!/bin/sh
set -e

APK_TOOLS_VERSION="__APK_TOOLS_VERSION__"
VELLUM_SHA256="__VELLUM_SHA256__"
APK_AARCH64_SHA256="__APK_AARCH64_SHA256__"
APK_ARMV7_SHA256="__APK_ARMV7_SHA256__"
SIGNING_KEY_SHA256="__SIGNING_KEY_SHA256__"

VELLUM_ROOT="/home/root/.vellum"
VELLUM_CLI_RELEASES="https://github.com/vellum-dev/vellum-cli/releases/latest/download"
VELLUM_PACKAGES_REPO="https://raw.githubusercontent.com/vellum-dev/vellum/main"
VELLUM_APK_RELEASES="https://github.com/vellum-dev/apk-tools/releases/download/$APK_TOOLS_VERSION"

verify_sha256() {
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

mkdir -p "$VELLUM_ROOT"/{bin,etc/apk/keys,lib/apk/db,share/bash-completion/completions,state,local-repo,cache}

echo "Downloading apk.vellum..."
wget -q "$VELLUM_APK_RELEASES/apk-$APK_ARCH" -O "$VELLUM_ROOT/bin/apk.vellum"
case "$APK_ARCH" in
    aarch64) verify_sha256 "$VELLUM_ROOT/bin/apk.vellum" "$APK_AARCH64_SHA256" ;;
    armv7)   verify_sha256 "$VELLUM_ROOT/bin/apk.vellum" "$APK_ARMV7_SHA256" ;;
esac
chmod +x "$VELLUM_ROOT/bin/apk.vellum"

echo "Downloading vellum..."
wget -q "$VELLUM_CLI_RELEASES/vellum" -O "$VELLUM_ROOT/bin/vellum"
verify_sha256 "$VELLUM_ROOT/bin/vellum" "$VELLUM_SHA256"
chmod +x "$VELLUM_ROOT/bin/vellum"

echo "Downloading signing key..."
wget -q "$VELLUM_PACKAGES_REPO/keys/packages.rsa.pub" -O "$VELLUM_ROOT/etc/apk/keys/packages.rsa.pub"
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
    --dest / \
    --no-logfile \
    add --initdb

echo "Updating package index..."
"$VELLUM_ROOT/bin/vellum" update

echo "Registering vellum package..."
"$VELLUM_ROOT/bin/apk.vellum" \
    --root "$VELLUM_ROOT" \
    --dest / \
    --force-no-chroot \
    --no-logfile \
    add vellum 2>/dev/null || true

echo "Installing bash completion..."
"$VELLUM_ROOT/bin/vellum" add vellum-bash-completion

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

echo ""
echo "Vellum installed successfully!"
echo "Run 'source ~/.bashrc' or start a new shell to use vellum."
