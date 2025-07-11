package testutil

import (
	"os"
	"strings"
)

// TestCase represents a single test case from a .test file
type TestCase struct {
	Name     string
	Input    string
	Expected string
	Type     string // "EVAL", "AST", "ERROR", or "TYPE_ERROR"
}

// ParseTestFile reads and parses a .test file into TestCase structs
func ParseTestFile(filepath string) ([]TestCase, error) {
	content, err := os.ReadFile(filepath)
	if err != nil {
		return nil, err
	}

	return parseTestFileContent(string(content)), nil
}

// parseTestFileContent parses the content of a .test file into TestCase structs
func parseTestFileContent(content string) []TestCase {
	var testCases []TestCase

	// Split by === to get individual test blocks
	blocks := strings.Split(content, "===")

	for _, block := range blocks {
		block = strings.TrimSpace(block)
		if block == "" {
			continue
		}

		testCase := parseTestBlock(block)
		if testCase != nil {
			testCases = append(testCases, *testCase)
		}
	}

	return testCases
}

// parseTestBlock parses a single test block into a TestCase
func parseTestBlock(block string) *TestCase {
	lines := strings.Split(block, "\n")

	var name, input, expected, testType string
	var currentSection string
	var inputLines, expectedLines []string

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		if strings.HasPrefix(line, "-- TEST:") {
			name = strings.TrimSpace(strings.TrimPrefix(line, "-- TEST:"))
			currentSection = "TEST"
		} else if line == "-- INPUT" {
			currentSection = "INPUT"
		} else if strings.HasPrefix(line, "-- EXPECT-") {
			if strings.HasPrefix(line, "-- EXPECT-EVAL") {
				testType = "EVAL"
			} else if strings.HasPrefix(line, "-- EXPECT-AST") {
				testType = "AST"
			} else if strings.HasPrefix(line, "-- EXPECT-ERROR") {
				testType = "ERROR"
			} else if strings.HasPrefix(line, "-- EXPECT-TYPE-ERROR") {
				testType = "TYPE_ERROR"
			}
			currentSection = "EXPECT"
		} else {
			switch currentSection {
			case "INPUT":
				inputLines = append(inputLines, line)
			case "EXPECT":
				expectedLines = append(expectedLines, line)
			}
		}
	}

	if name == "" || len(inputLines) == 0 || len(expectedLines) == 0 || testType == "" {
		return nil
	}

	input = strings.Join(inputLines, "\n")
	expected = strings.Join(expectedLines, "\n")

	return &TestCase{
		Name:     name,
		Input:    input,
		Expected: expected,
		Type:     testType,
	}
}
