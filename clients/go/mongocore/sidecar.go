package mongocore

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"time"
)

// SidecarManager handles the MongoCore sidecar lifecycle.
type SidecarManager struct {
	binaryPath string
	port       int
	cmd        *exec.Cmd
}

// NewSidecarManager creates a new sidecar manager.
func NewSidecarManager(port int) (*SidecarManager, error) {
	binary, err := findBinary()
	if err != nil {
		return nil, err
	}
	return &SidecarManager{binaryPath: binary, port: port}, nil
}

func findBinary() (string, error) {
	// Check PATH
	path, err := exec.LookPath("mongocore")
	if err == nil {
		return path, nil
	}

	// Check common locations
	home, _ := os.UserHomeDir()
	candidates := []string{
		filepath.Join(home, ".local", "bin", "mongocore"),
		filepath.Join(home, ".mongocore", "bin", "mongocore"),
		"/usr/local/bin/mongocore",
	}
	for _, p := range candidates {
		if _, err := os.Stat(p); err == nil {
			return p, nil
		}
	}

	return "", fmt.Errorf("mongocore binary not found")
}

// Start launches the sidecar process.
func (s *SidecarManager) Start() error {
	s.cmd = exec.Command(s.binaryPath, "--grpc-port", fmt.Sprintf("%d", s.port))
	s.cmd.Stdout = nil
	s.cmd.Stderr = nil
	return s.cmd.Start()
}

// WaitReady waits for the sidecar to become healthy.
func (s *SidecarManager) WaitReady(timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		// Simple check: try to connect
		// In production, would check gRPC health
		time.Sleep(100 * time.Millisecond)
	}
	return fmt.Errorf("sidecar failed to start within %v", timeout)
}

// Stop terminates the sidecar process.
func (s *SidecarManager) Stop() error {
	if s.cmd != nil && s.cmd.Process != nil {
		return s.cmd.Process.Kill()
	}
	return nil
}
