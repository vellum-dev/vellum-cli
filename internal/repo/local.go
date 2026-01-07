package repo

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"crypto"
	"crypto/rsa"
	"crypto/sha1"
	"crypto/x509"
	"encoding/base64"
	"encoding/pem"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

func UpdateIndex(repoDir, keyPath string) error {
	apks, err := filepath.Glob(filepath.Join(repoDir, "*.apk"))
	if err != nil {
		return err
	}

	var indexBuf bytes.Buffer
	for _, apkPath := range apks {
		if err := appendPackageToIndex(&indexBuf, apkPath); err != nil {
			return fmt.Errorf("processing %s: %w", apkPath, err)
		}
	}

	indexData := indexBuf.Bytes()

	var unsignedBuf bytes.Buffer
	gw := gzip.NewWriter(&unsignedBuf)
	tw := tar.NewWriter(gw)

	hdr := &tar.Header{
		Name: "APKINDEX",
		Mode: 0644,
		Size: int64(len(indexData)),
	}
	if err := tw.WriteHeader(hdr); err != nil {
		return err
	}
	if _, err := tw.Write(indexData); err != nil {
		return err
	}
	tw.Close()
	gw.Close()

	outputPath := filepath.Join(repoDir, "APKINDEX.tar.gz")

	if keyPath != "" {
		keyData, err := os.ReadFile(keyPath)
		if err == nil {
			return writeSignedIndex(outputPath, unsignedBuf.Bytes(), keyData)
		}
	}

	return os.WriteFile(outputPath, unsignedBuf.Bytes(), 0644)
}

func appendPackageToIndex(w io.Writer, apkPath string) error {
	f, err := os.Open(apkPath)
	if err != nil {
		return err
	}
	defer f.Close()

	stat, err := f.Stat()
	if err != nil {
		return err
	}
	size := stat.Size()

	data, err := io.ReadAll(f)
	if err != nil {
		return err
	}

	h := sha1.New()
	h.Write(data)
	cksum := base64.StdEncoding.EncodeToString(h.Sum(nil))

	gr, err := gzip.NewReader(bytes.NewReader(data))
	if err != nil {
		return err
	}
	defer gr.Close()

	tr := tar.NewReader(gr)
	var pkginfo []byte
	for {
		hdr, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return err
		}
		if hdr.Name == ".PKGINFO" {
			pkginfo, err = io.ReadAll(tr)
			if err != nil {
				return err
			}
			break
		}
	}

	if pkginfo == nil {
		return fmt.Errorf("no .PKGINFO found")
	}

	fmt.Fprintf(w, "C:Q1%s\n", cksum)

	for _, line := range strings.Split(string(pkginfo), "\n") {
		parts := strings.SplitN(line, "=", 2)
		if len(parts) != 2 {
			continue
		}
		key := strings.TrimSpace(parts[0])
		val := strings.TrimSpace(parts[1])
		switch key {
		case "pkgname":
			fmt.Fprintf(w, "P:%s\n", val)
		case "pkgver":
			fmt.Fprintf(w, "V:%s\n", val)
		case "arch":
			fmt.Fprintf(w, "A:%s\n", val)
		case "pkgdesc":
			fmt.Fprintf(w, "T:%s\n", val)
		case "url":
			fmt.Fprintf(w, "U:%s\n", val)
		case "license":
			fmt.Fprintf(w, "L:%s\n", val)
		case "provides":
			fmt.Fprintf(w, "p:%s\n", val)
		case "depends":
			fmt.Fprintf(w, "D:%s\n", val)
		}
	}

	fmt.Fprintf(w, "S:%d\n", size)
	fmt.Fprintf(w, "I:0\n")
	fmt.Fprintf(w, "\n")

	return nil
}

func writeSignedIndex(outputPath string, unsignedData, keyPEM []byte) error {
	block, _ := pem.Decode(keyPEM)
	if block == nil {
		return fmt.Errorf("failed to decode PEM key")
	}

	key, err := x509.ParsePKCS1PrivateKey(block.Bytes)
	if err != nil {
		pkcs8Key, err2 := x509.ParsePKCS8PrivateKey(block.Bytes)
		if err2 != nil {
			return fmt.Errorf("failed to parse private key: %v / %v", err, err2)
		}
		var ok bool
		key, ok = pkcs8Key.(*rsa.PrivateKey)
		if !ok {
			return fmt.Errorf("key is not RSA")
		}
	}

	h := sha1.New()
	h.Write(unsignedData)
	signature, err := rsa.SignPKCS1v15(nil, key, crypto.SHA1, h.Sum(nil))
	if err != nil {
		return err
	}

	var sigTarBuf bytes.Buffer
	tw := tar.NewWriter(&sigTarBuf)
	hdr := &tar.Header{
		Name: ".SIGN.RSA.local.rsa.pub",
		Mode: 0644,
		Size: int64(len(signature)),
	}
	if err := tw.WriteHeader(hdr); err != nil {
		return err
	}
	if _, err := tw.Write(signature); err != nil {
		return err
	}

	padSize := 512 - (len(signature)+512)%512
	if padSize < 512 {
		tw.Write(make([]byte, padSize))
	}
	tw.Close()

	contentBlocks := (512 + len(signature) + 511) / 512
	sigTarData := sigTarBuf.Bytes()[:contentBlocks*512]

	var sigGzBuf bytes.Buffer
	gw, _ := gzip.NewWriterLevel(&sigGzBuf, gzip.BestCompression)
	gw.Write(sigTarData)
	gw.Close()

	var finalBuf bytes.Buffer
	finalBuf.Write(sigGzBuf.Bytes())
	finalBuf.Write(unsignedData)

	return os.WriteFile(outputPath, finalBuf.Bytes(), 0644)
}
