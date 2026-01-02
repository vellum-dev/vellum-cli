# vellum-cli

Wrapper and boostrap for the Vellum reMarkable package manager. Wraps Alpine's `apk` to work around reMarkable's read-only root filesystem.

## Installation

```sh
wget https://github.com/vellum-dev/vellum-cli/releases/latest/download/bootstrap.sh
echo "00fc77169833e3a7f5a8dd66ed8029d47055f7063e3f8aa2e627115665493f43  bootstrap.sh" | sha256sum -c && bash bootstrap.sh
```

## Usage

```sh
vellum add <package>       # Install a package
vellum del <package>       # Remove a package
vellum update              # Update package index
vellum upgrade             # Upgrade installed packages
vellum search <query>      # Search for packages
vellum info <package>      # Show package details
vellum self uninstall      # Uninstall vellum
```

Most `apk` commands are passed through directly.

## How it works

- Keeps all package manager state in `/home/root/.vellum/`
- Generates virtual packages for device detection (`rmpp`, `rm2`, etc.) and OS version (`remarkable-os`)
- Uses a local package repository for virtual packages
- Passes through to a statically-linked `apk` binary

## Related repositories

- [vellum](https://github.com/vellum-dev/vellum) - Package registry (APKBUILDs)
- [apk-tools](https://github.com/vellum-dev/apk-tools) - Static apk binary

## License

MIT
