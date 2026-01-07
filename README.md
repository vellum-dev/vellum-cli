# vellum-cli

Package manager for reMarkable tablets. Wraps Alpine's `apk` to work around reMarkable's read-only root filesystem.

## Installation

```sh
wget https://github.com/vellum-dev/vellum-cli/releases/latest/download/bootstrap.sh
echo "463e4cd96faa8cbeb1f432e2bc43f2c5dcf6c2fd5f7e4d471df8658cf76fe3e3  bootstrap.sh" | sha256sum -c && bash bootstrap.sh
```

## Usage

```sh
vellum add <package>       # Install a package
vellum del <package>       # Remove a package
vellum update              # Update package index
vellum upgrade             # Upgrade installed packages
vellum search <query>      # Search for packages
vellum info <package>      # Show package details
vellum check-os <version>  # Check package compatibility with an OS version
vellum reenable            # Restore system files after OS upgrade
vellum self uninstall      # Uninstall vellum (--all to include packages)
```

Most `apk` commands are passed through directly.

### OS Compatibility

Before upgrading your reMarkable OS, check if installed packages will still work:
```sh
vellum check-os 3.24.0.149
```

After an OS upgrade, vellum detects the version change and requires `vellum upgrade` to sync packages.

## How it works

- Keeps all package manager state in `/home/root/.vellum/`
- Generates virtual packages for device detection (`rmpp`, `rm2`, etc.) and OS version (`remarkable-os`)
- Checks package compatibility before OS upgrades
- Uses a local package repository for virtual packages
- Passes through to a statically-linked `apk` binary

## Building

```sh
GOOS=linux GOARCH=arm64 go build -o vellum-arm64 ./cmd/vellum
GOOS=linux GOARCH=arm go build -o vellum-armv7 ./cmd/vellum
```

## Related repositories

- [vellum](https://github.com/vellum-dev/vellum) - Package registry (APKBUILDs)
- [apk-tools](https://github.com/vellum-dev/apk-tools) - Static apk binary

## License

MIT
