package apk

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"fmt"
	"os"
	"path/filepath"
)

func GenerateRemarkableOSPackage(version, repoDir string) error {
	if err := os.MkdirAll(repoDir, 0755); err != nil {
		return err
	}

	pkginfo := fmt.Sprintf(`pkgname = remarkable-os
pkgver = %s-r0
pkgdesc = Virtual package representing reMarkable OS version
url = https://github.com/vellum-dev/vellum-cli
arch = noarch
license = MIT
provides = /bin/sh
`, version)

	return writePackage(repoDir, fmt.Sprintf("remarkable-os-%s-r0.apk", version), pkginfo)
}

func GenerateDevicePackage(device, repoDir string) error {
	if err := os.MkdirAll(repoDir, 0755); err != nil {
		return err
	}

	var desc string
	switch device {
	case "rmpp":
		desc = "reMarkable Paper Pro"
	case "rmppm":
		desc = "reMarkable Paper Pro Move"
	case "rm2":
		desc = "reMarkable 2"
	case "rm1":
		desc = "reMarkable 1"
	default:
		desc = "reMarkable Device"
	}

	pkginfo := fmt.Sprintf(`pkgname = %s
pkgver = 1.0.0-r0
pkgdesc = Virtual package for %s
url = https://github.com/vellum-dev/vellum-cli
arch = noarch
license = MIT
`, device, desc)

	return writePackage(repoDir, fmt.Sprintf("%s-1.0.0-r0.apk", device), pkginfo)
}

func writePackage(repoDir, filename, pkginfo string) error {
	var dataBuf bytes.Buffer
	gw := gzip.NewWriter(&dataBuf)
	tw := tar.NewWriter(gw)

	hdr := &tar.Header{
		Name:     "./",
		Mode:     0755,
		Typeflag: tar.TypeDir,
	}
	if err := tw.WriteHeader(hdr); err != nil {
		return err
	}
	tw.Close()
	gw.Close()

	var apkBuf bytes.Buffer
	gw = gzip.NewWriter(&apkBuf)
	tw = tar.NewWriter(gw)

	pkginfoBytes := []byte(pkginfo)
	hdr = &tar.Header{
		Name: ".PKGINFO",
		Mode: 0644,
		Size: int64(len(pkginfoBytes)),
	}
	if err := tw.WriteHeader(hdr); err != nil {
		return err
	}
	if _, err := tw.Write(pkginfoBytes); err != nil {
		return err
	}

	dataBytes := dataBuf.Bytes()
	hdr = &tar.Header{
		Name: "data.tar.gz",
		Mode: 0644,
		Size: int64(len(dataBytes)),
	}
	if err := tw.WriteHeader(hdr); err != nil {
		return err
	}
	if _, err := tw.Write(dataBytes); err != nil {
		return err
	}

	tw.Close()
	gw.Close()

	return os.WriteFile(filepath.Join(repoDir, filename), apkBuf.Bytes(), 0644)
}
