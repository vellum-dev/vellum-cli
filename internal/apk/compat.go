package apk

type CompatResult struct {
	Compatible   []string
	Incompatible []string
	FetchFailed  bool
}

func CheckOSCompatibility(targetOS string, installedPkgs []string, index []Package) CompatResult {
	result := CompatResult{
		Compatible:   []string{},
		Incompatible: []string{},
	}

	pkgVersions := make(map[string][]Package)
	for _, pkg := range index {
		pkgVersions[pkg.Name] = append(pkgVersions[pkg.Name], pkg)
	}

	for _, installed := range installedPkgs {
		versions, exists := pkgVersions[installed]
		if !exists {
			continue
		}

		hasOS := false
		for _, v := range versions {
			min, max := v.GetOSConstraints()
			if min != "" || max != "" {
				hasOS = true
				break
			}
		}
		if !hasOS {
			continue
		}

		hasCompatible := false
		for _, v := range versions {
			if v.IsCompatibleWithOS(targetOS) {
				hasCompatible = true
				break
			}
		}

		if hasCompatible {
			result.Compatible = append(result.Compatible, installed)
		} else {
			result.Incompatible = append(result.Incompatible, installed)
		}
	}

	return result
}
