package main

import (
	"bufio"
	"fmt"
	"os"
	osexec "os/exec"
	"path/filepath"
	"strings"

	"github.com/vellum-dev/vellum-cli/internal/apk"
	"github.com/vellum-dev/vellum-cli/internal/device"
	"github.com/vellum-dev/vellum-cli/internal/exec"
	"github.com/vellum-dev/vellum-cli/internal/repo"
	"github.com/vellum-dev/vellum-cli/internal/state"
)

const vellumRoot = "/home/root/.vellum"

var (
	osMismatch bool
	osCur      string
	osPrev     string
)

func main() {
	st := state.New(vellumRoot)
	apkExec := exec.NewAPK(vellumRoot)

	ensureRemarkableOS(st, apkExec)
	ensureDevicePackage(st, apkExec)

	if len(os.Args) < 2 {
		showHelp(apkExec)
		return
	}

	cmd := os.Args[1]

	if osMismatch && !isAllowedDuringMismatch(cmd) {
		fmt.Println()
		fmt.Printf("OS upgraded (%s → %s).\n", osPrev, osCur)
		fmt.Println("Run 'vellum upgrade' to sync packages with new OS version.")
		fmt.Println()
		os.Exit(1)
	}

	switch cmd {
	case "--help", "-h":
		showHelp(apkExec)
	case "install":
		handleAdd(apkExec, os.Args[2:])
	case "remove":
		handleDel(apkExec, os.Args[2:])
	case "purge":
		handlePurge(apkExec, os.Args[2:])
	case "show":
		handleShow(apkExec, os.Args[2:])
	case "add":
		handleAdd(apkExec, os.Args[2:])
	case "del":
		handleDel(apkExec, os.Args[2:])
	case "upgrade":
		handleUpgrade(st, apkExec, os.Args[2:])
	case "reenable":
		handleReenable()
	case "check-os":
		if len(os.Args) < 3 {
			fmt.Println("Usage: vellum check-os <version>")
			fmt.Println("Check if installed packages are compatible with a given OS version.")
			os.Exit(1)
		}
		handleCheckOS(apkExec, os.Args[2])
	case "self":
		if len(os.Args) > 2 && os.Args[2] == "uninstall" {
			handleSelfUninstall(apkExec, os.Args[3:])
		} else {
			fmt.Println("Unknown self command")
			fmt.Println("Usage: vellum self uninstall [--all] [--yes]")
			os.Exit(1)
		}
	default:
		if err := apkExec.Exec(os.Args[1:]...); err != nil {
			fmt.Fprintf(os.Stderr, "exec error: %v\n", err)
			os.Exit(1)
		}
	}
}

func isAllowedDuringMismatch(cmd string) bool {
	switch cmd {
	case "upgrade", "del", "remove", "purge", "--help", "-h", "self":
		return true
	}
	return false
}

func ensureRemarkableOS(st *state.State, apkExec *exec.APK) {
	var err error
	osCur, err = device.GetOSVersion()
	if err != nil {
		return
	}

	osPrev, _ = st.GetOSVersion()
	arch := device.GetAPKArch()
	repoDir := filepath.Join(vellumRoot, "local-repo", arch)
	keyPath := filepath.Join(vellumRoot, "etc", "apk", "keys", "local.rsa")

	if osPrev == "" {
		os.MkdirAll(repoDir, 0755)
		removeGlob(filepath.Join(repoDir, "remarkable-os-*.apk"))
		apk.GenerateRemarkableOSPackage(osCur, repoDir)
		repo.UpdateIndex(repoDir, keyPath)
		st.SetOSVersion(osCur)
		apkExec.RunSilent("add", "remarkable-os")
	} else if osCur != osPrev {
		osMismatch = true
	}
}

func ensureDevicePackage(st *state.State, apkExec *exec.APK) {
	deviceType := device.GetDeviceType()
	prevDevice, _ := st.GetDevice()
	arch := device.GetAPKArch()
	repoDir := filepath.Join(vellumRoot, "local-repo", arch)
	keyPath := filepath.Join(vellumRoot, "etc", "apk", "keys", "local.rsa")

	pkgPath := filepath.Join(repoDir, deviceType+"-1.0.0-r0.apk")
	if deviceType != prevDevice || !fileExists(pkgPath) {
		os.MkdirAll(repoDir, 0755)
		for _, d := range []string{"rm1", "rm2", "rmpp", "rmppm"} {
			removeGlob(filepath.Join(repoDir, d+"-*.apk"))
		}
		apk.GenerateDevicePackage(deviceType, repoDir)
		repo.UpdateIndex(repoDir, keyPath)
		st.SetDevice(deviceType)
		apkExec.RunSilent("add", deviceType)
	}
}

func showHelp(apkExec *exec.APK) {
	fmt.Println(`Vellum package manager for reMarkable

Usage: vellum <command> [options]

Vellum commands:
  upgrade             Upgrade packages (handles OS version changes)
  check-os <version>  Check package compatibility with an OS version
  reenable            Restore system files after OS upgrade
  self uninstall      Remove vellum itself (--all to include packages)

Aliases:
  install <pkg>       Alias for 'add'
  remove <pkg>        Alias for 'del'
  purge <pkg>         Alias for 'del --purge'
  show <pkg>          Alias for 'info -a'
`)
	apkExec.Run("--help")
}

func handleAdd(apkExec *exec.APK, args []string) {
	for _, arg := range args {
		if arg == "vellum" {
			fmt.Println("Error: Cannot add/remove vellum package directly.")
			fmt.Println("Use 'vellum self uninstall' to remove vellum.")
			os.Exit(1)
		}
	}
	err := apkExec.Run(append([]string{"add", "--cache-predownload"}, args...)...)
	apkExec.CachePurge()
	if err != nil {
		os.Exit(1)
	}
}

func handleDel(apkExec *exec.APK, args []string) {
	for _, arg := range args {
		if arg == "vellum" {
			fmt.Println("Error: Cannot add/remove vellum package directly.")
			fmt.Println("Use 'vellum self uninstall' to remove vellum.")
			os.Exit(1)
		}
	}
	if err := apkExec.Run(append([]string{"del"}, args...)...); err != nil {
		os.Exit(1)
	}
}

func handlePurge(apkExec *exec.APK, args []string) {
	for _, arg := range args {
		if arg == "vellum" {
			fmt.Println("Error: Cannot add/remove vellum package directly.")
			fmt.Println("Use 'vellum self uninstall' to remove vellum.")
			os.Exit(1)
		}
	}
	os.Setenv("VELLUM_PURGE", "1")
	if err := apkExec.Run(append([]string{"del", "--purge"}, args...)...); err != nil {
		os.Exit(1)
	}
}

func handleShow(apkExec *exec.APK, args []string) {
	if err := apkExec.Run(append([]string{"info", "-a"}, args...)...); err != nil {
		os.Exit(1)
	}
}

func handleUpgrade(st *state.State, apkExec *exec.APK, args []string) {
	if osMismatch {
		fmt.Printf("OS upgraded (%s → %s). Checking package compatibility...\n", osPrev, osCur)
		fmt.Println()

		incompatible := checkOSCompatibility(apkExec, osCur)
		if incompatible == nil {
			fmt.Println("Could not fetch package index to verify compatibility.")
			fmt.Println("Check your network connection and try again.")
			os.Exit(1)
		}

		if len(incompatible) > 0 {
			fmt.Printf("These packages have no version compatible with OS %s:\n", osCur)
			for _, pkg := range incompatible {
				fmt.Printf("  - %s\n", pkg)
			}
			fmt.Println()
			fmt.Println("Either wait for them to be updated, or remove them with 'vellum del <package>'.")
			fmt.Println("Then run 'vellum upgrade' again.")
			os.Exit(1)
		}

		fmt.Println("All packages have compatible versions. Syncing OS version...")

		arch := device.GetAPKArch()
		repoDir := filepath.Join(vellumRoot, "local-repo", arch)
		keyPath := filepath.Join(vellumRoot, "etc", "apk", "keys", "local.rsa")

		os.MkdirAll(repoDir, 0755)
		removeGlob(filepath.Join(repoDir, "remarkable-os-*.apk"))
		apk.GenerateRemarkableOSPackage(osCur, repoDir)
		repo.UpdateIndex(repoDir, keyPath)
		st.SetOSVersion(osCur)
		apkExec.RunSilent("add", "remarkable-os")

		fmt.Println("Upgrading packages...")
	}

	if err := apkExec.Exec(append([]string{"upgrade"}, args...)...); err != nil {
		fmt.Fprintf(os.Stderr, "exec error: %v\n", err)
		os.Exit(1)
	}
}

func handleCheckOS(apkExec *exec.APK, targetOS string) {
	fmt.Printf("Checking package compatibility with OS %s...\n\n", targetOS)

	installed, err := apkExec.ListInstalled()
	if err != nil {
		fmt.Println("Could not list installed packages.")
		os.Exit(1)
	}

	virtualPkgs := map[string]bool{
		"remarkable-os": true,
		"rm1":           true,
		"rm2":           true,
		"rmpp":          true,
		"rmppm":         true,
	}

	var userPkgs []string
	for _, pkg := range installed {
		if !virtualPkgs[pkg] {
			userPkgs = append(userPkgs, pkg)
		}
	}

	if len(userPkgs) == 0 {
		fmt.Println("No user packages installed.")
		return
	}

	repoURL := getRepoURL()
	if repoURL == "" {
		fmt.Println("Could not determine repository URL.")
		os.Exit(1)
	}

	arch := device.GetAPKArch()
	index, err := apk.FetchRemoteIndex(repoURL, arch)
	if err != nil {
		fmt.Println("Could not fetch package index.")
		fmt.Printf("Error: %v\n", err)
		os.Exit(1)
	}

	pkgVersions := make(map[string][]apk.Package)
	for _, pkg := range index {
		pkgVersions[pkg.Name] = append(pkgVersions[pkg.Name], pkg)
	}

	var compatible, incompatible, noConstraint []string

	for _, pkg := range userPkgs {
		versions, exists := pkgVersions[pkg]
		if !exists {
			continue
		}

		hasOSConstraint := false
		hasCompatibleVersion := false

		for _, v := range versions {
			min, max := v.GetOSConstraints()
			if min != "" || max != "" {
				hasOSConstraint = true
				if v.IsCompatibleWithOS(targetOS) {
					hasCompatibleVersion = true
					break
				}
			}
		}

		if !hasOSConstraint {
			noConstraint = append(noConstraint, pkg)
		} else if hasCompatibleVersion {
			compatible = append(compatible, pkg)
		} else {
			incompatible = append(incompatible, pkg)
		}
	}

	if len(compatible) > 0 {
		fmt.Println("Compatible packages:")
		for _, pkg := range compatible {
			fmt.Printf("  ✓ %s\n", pkg)
		}
		fmt.Println()
	}

	if len(noConstraint) > 0 {
		fmt.Println("Packages without OS constraints (assumed compatible):")
		for _, pkg := range noConstraint {
			fmt.Printf("  - %s\n", pkg)
		}
		fmt.Println()
	}

	if len(incompatible) > 0 {
		fmt.Println("Incompatible packages (no version available for this OS):")
		for _, pkg := range incompatible {
			fmt.Printf("  ✗ %s\n", pkg)
		}
		fmt.Println()
		os.Exit(1)
	}

	fmt.Println("All packages are compatible.")
}

func checkOSCompatibility(apkExec *exec.APK, targetOS string) []string {
	installed, err := apkExec.ListInstalled()
	if err != nil {
		return nil
	}

	virtualPkgs := map[string]bool{
		"remarkable-os": true,
		"rm1":           true,
		"rm2":           true,
		"rmpp":          true,
		"rmppm":         true,
	}

	var filteredInstalled []string
	for _, pkg := range installed {
		if !virtualPkgs[pkg] {
			filteredInstalled = append(filteredInstalled, pkg)
		}
	}

	if len(filteredInstalled) == 0 {
		return []string{}
	}

	repoURL := getRepoURL()
	if repoURL == "" {
		return nil
	}

	arch := device.GetAPKArch()
	index, err := apk.FetchRemoteIndex(repoURL, arch)
	if err != nil {
		return nil
	}

	var installedWithOSDep []string
	for _, pkg := range filteredInstalled {
		deps, err := apkExec.GetDependencies(pkg)
		if err != nil {
			continue
		}
		for _, dep := range deps {
			if strings.Contains(dep, "remarkable-os") {
				installedWithOSDep = append(installedWithOSDep, pkg)
				break
			}
		}
	}

	if len(installedWithOSDep) == 0 {
		return []string{}
	}

	result := apk.CheckOSCompatibility(targetOS, installedWithOSDep, index)
	return result.Incompatible
}

func getRepoURL() string {
	reposFile := filepath.Join(vellumRoot, "etc", "apk", "repositories")
	f, err := os.Open(reposFile)
	if err != nil {
		return ""
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if !strings.Contains(line, "local-repo") {
			return line
		}
	}
	return ""
}

func handleReenable() {
	hooksDir := filepath.Join(vellumRoot, "hooks", "post-os-upgrade")
	entries, err := os.ReadDir(hooksDir)
	if err != nil || len(entries) == 0 {
		fmt.Println("No packages require re-enabling after OS upgrades.")
		os.Exit(0)
	}

	fmt.Println("Re-enabling packages after OS upgrade...")

	mountRW := filepath.Join(vellumRoot, "bin", "mount-rw")
	mountRestore := filepath.Join(vellumRoot, "bin", "mount-restore")

	runCommand(mountRW)
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		scriptPath := filepath.Join(hooksDir, entry.Name())
		info, err := os.Stat(scriptPath)
		if err != nil || info.Mode()&0111 == 0 {
			continue
		}
		fmt.Printf("  %s\n", entry.Name())
		if err := runCommand(scriptPath); err != nil {
			fmt.Printf("    warning: %s reenable script failed\n", entry.Name())
		}
	}
	runCommand(mountRestore)

	fmt.Println("Done.")
}

func handleSelfUninstall(apkExec *exec.APK, args []string) {
	uninstallAll := false
	uninstallYes := false

	for _, arg := range args {
		switch arg {
		case "--all":
			uninstallAll = true
		case "--yes", "-y":
			uninstallYes = true
		}
	}

	if !uninstallYes {
		msg := "This will remove vellum"
		if uninstallAll {
			msg += " and permanently delete all installed packages and their data"
		}
		fmt.Printf("%s. Continue? [y/N] ", msg)

		reader := bufio.NewReader(os.Stdin)
		confirm, _ := reader.ReadString('\n')
		confirm = strings.TrimSpace(strings.ToLower(confirm))
		if confirm != "y" && confirm != "yes" {
			fmt.Println("Aborted.")
			os.Exit(1)
		}
	}

	if uninstallAll {
		fmt.Println("Removing all installed packages...")
		os.Setenv("VELLUM_PURGE", "1")
		installed, _ := apkExec.ListInstalled()
		for _, pkg := range installed {
			apkExec.RunSilent("del", "--purge", pkg)
		}
	}

	fmt.Println("Removing vellum...")

	bashrc := filepath.Join(os.Getenv("HOME"), ".bashrc")
	if data, err := os.ReadFile(bashrc); err == nil {
		lines := strings.Split(string(data), "\n")
		var newLines []string
		for _, line := range lines {
			if !strings.Contains(line, ".vellum") {
				newLines = append(newLines, line)
			}
		}
		os.WriteFile(bashrc, []byte(strings.Join(newLines, "\n")), 0644)
	}

	os.RemoveAll(vellumRoot)
	fmt.Println("Vellum has been removed.")
}

func runCommand(path string) error {
	cmd := osexec.Command(path)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}

func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func removeGlob(pattern string) {
	matches, _ := filepath.Glob(pattern)
	for _, m := range matches {
		os.Remove(m)
	}
}
