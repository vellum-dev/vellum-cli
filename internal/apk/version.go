package apk

import (
	"strconv"
	"strings"
)

func CompareVersions(a, b string) int {
	if a == b {
		return 0
	}

	aParts := strings.Split(a, ".")
	bParts := strings.Split(b, ".")

	minLen := len(aParts)
	if len(bParts) < minLen {
		minLen = len(bParts)
	}

	for i := 0; i < minLen; i++ {
		aNum, _ := strconv.Atoi(strings.Split(aParts[i], "-")[0])
		bNum, _ := strconv.Atoi(strings.Split(bParts[i], "-")[0])

		if aNum > bNum {
			return 1
		}
		if aNum < bNum {
			return -1
		}
	}

	if len(aParts) > len(bParts) {
		return 1
	}
	if len(bParts) > len(aParts) {
		return -1
	}
	return 0
}

func VersionGTE(a, b string) bool {
	return CompareVersions(a, b) >= 0
}

func VersionLT(a, b string) bool {
	return CompareVersions(a, b) < 0
}
