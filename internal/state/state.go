package state

import (
	"os"
	"path/filepath"
	"strings"
)

type State struct {
	root string
}

func New(vellumRoot string) *State {
	return &State{root: vellumRoot}
}

func (s *State) dir() string {
	return filepath.Join(s.root, "state")
}

func (s *State) GetOSVersion() (string, error) {
	data, err := os.ReadFile(filepath.Join(s.dir(), "osver"))
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(data)), nil
}

func (s *State) SetOSVersion(version string) error {
	if err := os.MkdirAll(s.dir(), 0755); err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(s.dir(), "osver"), []byte(version), 0644)
}

func (s *State) GetDevice() (string, error) {
	data, err := os.ReadFile(filepath.Join(s.dir(), "device"))
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(data)), nil
}

func (s *State) SetDevice(device string) error {
	if err := os.MkdirAll(s.dir(), 0755); err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(s.dir(), "device"), []byte(device), 0644)
}
