package apk

import (
	"archive/tar"
	"bufio"
	"bytes"
	"compress/gzip"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
)

type Package struct {
	Name    string
	Version string
	Depends []string
}

func ParseIndexFile(path string) ([]Package, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	return parseAPKINDEX(f)
}

func ParseIndexTarGz(path string) ([]Package, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	return parseIndexFromTarGz(f)
}

func FetchRemoteIndex(repoURL, arch string) ([]Package, error) {
	url := fmt.Sprintf("%s/%s/APKINDEX.tar.gz", strings.TrimRight(repoURL, "/"), arch)

	resp, err := http.Get(url)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("HTTP %d", resp.StatusCode)
	}

	return parseIndexFromTarGz(resp.Body)
}

func parseIndexFromTarGz(r io.Reader) ([]Package, error) {
	data, err := io.ReadAll(r)
	if err != nil {
		return nil, err
	}

	reader := bytes.NewReader(data)

	for reader.Len() > 0 {
		gr, err := gzip.NewReader(reader)
		if err != nil {
			break
		}

		tr := tar.NewReader(gr)
		for {
			hdr, err := tr.Next()
			if err == io.EOF {
				break
			}
			if err != nil {
				break
			}
			if hdr.Name == "APKINDEX" {
				packages, err := parseAPKINDEX(tr)
				gr.Close()
				return packages, err
			}
		}
		gr.Close()
	}

	return nil, fmt.Errorf("APKINDEX not found in archive")
}

func parseAPKINDEX(r io.Reader) ([]Package, error) {
	var packages []Package
	var current Package

	scanner := bufio.NewScanner(r)
	for scanner.Scan() {
		line := scanner.Text()

		if line == "" {
			if current.Name != "" {
				packages = append(packages, current)
			}
			current = Package{}
			continue
		}

		if len(line) < 2 || line[1] != ':' {
			continue
		}

		key := line[0]
		val := line[2:]

		switch key {
		case 'P':
			current.Name = val
		case 'V':
			current.Version = val
		case 'D':
			current.Depends = strings.Fields(val)
		}
	}

	if current.Name != "" {
		packages = append(packages, current)
	}

	return packages, scanner.Err()
}

func (p *Package) GetOSConstraints() (minVer, maxVer string) {
	for _, dep := range p.Depends {
		if strings.HasPrefix(dep, "remarkable-os>=") {
			minVer = strings.TrimPrefix(dep, "remarkable-os>=")
		} else if strings.HasPrefix(dep, "remarkable-os<") {
			maxVer = strings.TrimPrefix(dep, "remarkable-os<")
		}
	}
	return
}

func (p *Package) IsCompatibleWithOS(osVersion string) bool {
	minVer, maxVer := p.GetOSConstraints()
	if minVer == "" && maxVer == "" {
		return true
	}
	if minVer != "" && !VersionGTE(osVersion, minVer) {
		return false
	}
	if maxVer != "" && !VersionLT(osVersion, maxVer) {
		return false
	}
	return true
}
