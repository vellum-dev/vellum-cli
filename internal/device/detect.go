package device

import (
	"os"
	"os/exec"
	"runtime"
	"strings"
)

func GetOSVersion() (string, error) {
	data, err := os.ReadFile("/usr/share/remarkable/update.conf")
	if err == nil {
		for _, line := range strings.Split(string(data), "\n") {
			if strings.HasPrefix(line, "RELEASE_VERSION=") {
				ver := strings.TrimPrefix(line, "RELEASE_VERSION=")
				ver = strings.Trim(ver, `"'`)
				if ver != "" {
					return ver, nil
				}
			}
		}
	}

	data, err = os.ReadFile("/etc/os-release")
	if err == nil {
		for _, line := range strings.Split(string(data), "\n") {
			if strings.HasPrefix(line, "IMG_VERSION=") {
				ver := strings.TrimPrefix(line, "IMG_VERSION=")
				ver = strings.Trim(ver, `"'`)
				if ver != "" {
					return ver, nil
				}
			}
		}
	}

	return "", os.ErrNotExist
}

func GetAPKArch() string {
	arch := runtime.GOARCH
	if arch == "arm64" {
		return "aarch64"
	}

	out, err := exec.Command("uname", "-m").Output()
	if err == nil {
		m := strings.TrimSpace(string(out))
		switch m {
		case "aarch64":
			return "aarch64"
		case "armv7l":
			return "armv7"
		}
	}
	return "noarch"
}

func GetDeviceType() string {
	data, err := os.ReadFile("/sys/devices/soc0/machine")
	if err != nil {
		return "rm2"
	}
	machine := strings.TrimSpace(string(data))
	switch {
	case strings.Contains(machine, "Ferrari"):
		return "rmpp"
	case strings.Contains(machine, "Chiappa"):
		return "rmppm"
	default:
		return "rm2"
	}
}
