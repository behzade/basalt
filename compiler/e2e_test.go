package main_test

import (
	"bytes"
	"io/ioutil"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestEndToEnd(t *testing.T) {
	// Get the current working directory
	wd, err := os.Getwd()
	if err != nil {
		t.Fatalf("Failed to get current working directory: %v", err)
	}

	// Construct the path to the compiler executable
	compilerPath := filepath.Join(wd, "main.go")

	// Define test cases
	tests := []struct {
		name     string
		filePath string
		expected string
	}{
		{
			name:     "functions",
			filePath: filepath.Join(filepath.Dir(wd), "tests", "functions", "main.zl"),
			expected: "Result: 0",
		},
		{
			name:     "control_flow",
			filePath: filepath.Join(filepath.Dir(wd), "tests", "control_flow", "main.zl"),
			expected: "Result: 0",
		},
		{
			name:     "operators",
			filePath: filepath.Join(filepath.Dir(wd), "tests", "operators", "main.zl"),
			expected: "Result: 0",
		},
		{
			name:     "variables",
			filePath: filepath.Join(filepath.Dir(wd), "tests", "variables", "main.zl"),
			expected: "Result: 0",
		},
		{
			name:     "return_operator_expression",
			filePath: filepath.Join(filepath.Dir(wd), "tests", "return_operator_expression", "main.zl"),
			expected: "Result: 10",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Read the content of the .zl file
			input, err := ioutil.ReadFile(tt.filePath)
			if err != nil {
				t.Fatalf("Failed to read test file %s: %v", tt.filePath, err)
			}

			// Create a command to run the compiler with the .zl file content as stdin
			cmd := exec.Command("go", "run", compilerPath)
			cmd.Stdin = bytes.NewReader(input)

			// Capture stdout and stderr
			var stdout, stderr bytes.Buffer
			cmd.Stdout = &stdout
			cmd.Stderr = &stderr

			// Run the command
			err = cmd.Run()
			if err != nil {
				t.Fatalf("Compiler command failed with error: %v\nStderr: %s", err, stderr.String())
			}

			// Compare the actual output with the expected output
			actual := strings.TrimSpace(stdout.String())
			if actual != tt.expected {
				t.Errorf("Output mismatch for %s\nExpected: %q\nActual:   %q", tt.name, tt.expected, actual)
			}
		})
	}
}
