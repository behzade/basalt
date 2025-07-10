#!/bin/bash

export CGO_CFLAGS="$(/opt/homebrew/opt/llvm/bin/llvm-config --cflags)"
export CGO_LDFLAGS="$(/opt/homebrew/opt/llvm/bin/llvm-config --ldflags --libs core)"

if [ -z "$1" ]; then
  # No argument provided, assume piped input
  go run ./compiler
else
  # Argument provided, assume it's a file path
  cat "$1" | go run ./compiler
fi