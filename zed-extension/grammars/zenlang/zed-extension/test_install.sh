#!/bin/bash

echo "=== Testing Zenlang Zed Extension ==="
echo

echo "1. Checking extension.toml structure..."
cat extension.toml
echo

echo "2. Checking grammar directory structure..."
l -ld grammars/zenlang | grep -E "\.git|grammar.js|tree-sitter|package.json"
echo

echo "3. Checking git status of grammar..."
git status --porcelain | head -5
echo

echo "4. Checking git log for grammar..."
git log --oneline -1
echo

echo "5. Current directory:"
pwd
echo

echo "6. Checking if tree-sitter.json exists..."
[ -f "grammars/zenlang/tree-sitter.json" ] && echo "✓ tree-sitter.json exists" || echo "✗ tree-sitter.json missing"
echo

echo "7. Checking grammar.js syntax..."
grep "name.*zenlang" grammars/zenlang/grammar.js
echo

echo "8. Checking git config..."
git config user.email
git config user.name