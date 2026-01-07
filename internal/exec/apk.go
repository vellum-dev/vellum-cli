package exec

import (
	"bytes"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
)

type APK struct {
	root string
}

func NewAPK(vellumRoot string) *APK {
	return &APK{root: vellumRoot}
}

func (a *APK) binPath() string {
	return filepath.Join(a.root, "bin", "apk.vellum")
}

func (a *APK) baseArgs() []string {
	return []string{
		"--root", a.root,
		"--dest", "/",
		"--force-no-chroot",
		"--no-logfile",
	}
}

func (a *APK) Run(args ...string) error {
	cmd := exec.Command(a.binPath(), append(a.baseArgs(), args...)...)
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Env = append(os.Environ(), "APK_CONFIG="+filepath.Join(a.root, "etc", "apk", "config"))
	return cmd.Run()
}

func (a *APK) RunSilent(args ...string) error {
	cmd := exec.Command(a.binPath(), append(a.baseArgs(), args...)...)
	cmd.Env = append(os.Environ(), "APK_CONFIG="+filepath.Join(a.root, "etc", "apk", "config"))
	return cmd.Run()
}

func (a *APK) Output(args ...string) (string, error) {
	cmd := exec.Command(a.binPath(), append(a.baseArgs(), args...)...)
	cmd.Env = append(os.Environ(), "APK_CONFIG="+filepath.Join(a.root, "etc", "apk", "config"))
	var out bytes.Buffer
	cmd.Stdout = &out
	err := cmd.Run()
	return strings.TrimSpace(out.String()), err
}

func (a *APK) Exec(args ...string) error {
	bin := a.binPath()
	allArgs := append([]string{bin}, append(a.baseArgs(), args...)...)
	env := append(os.Environ(), "APK_CONFIG="+filepath.Join(a.root, "etc", "apk", "config"))
	return syscall.Exec(bin, allArgs, env)
}

func (a *APK) ListInstalled() ([]string, error) {
	out, err := a.Output("info", "-q")
	if err != nil {
		return nil, err
	}
	if out == "" {
		return nil, nil
	}
	return strings.Split(out, "\n"), nil
}

func (a *APK) GetDependencies(pkg string) ([]string, error) {
	out, err := a.Output("info", "-R", pkg)
	if err != nil {
		return nil, err
	}
	return strings.Split(out, "\n"), nil
}

func (a *APK) CachePurge() error {
	return a.RunSilent("cache", "purge")
}
