# Basalt Import System - Remaining Tasks

## ✅ Completed
- [x] Import syntax parsing (`import Std::Fmt;`, `import Std::Collections as Collections;`)
- [x] Public symbol system (`pub` keyword for functions, structs, etc.)
- [x] Import mapping and path resolution (`Fmt::println` → `Std::Fmt::println`)
- [x] Module symbol caching in type checker
- [x] Mock module implementations for testing
- [x] Basic type checking with imported symbols

## 🔧 Remaining Tasks

### 1. Real File System Module Loading
**Priority: High**

Replace the mock `load_module_symbols` implementation with actual file system loading:

```rust
fn load_module_symbols(&mut self, namespace: &str, module: &str) -> Option<HashMap<&'src str, SymbolSignature<'src>>> {
    // TODO: Implement real file loading
    // 1. Determine module path: "./modules/{namespace}/{module}/" or "./{module}/" for Self
    // 2. Read all .bst files in the directory
    // 3. Parse each file and collect only public symbols
    // 4. Return symbol signatures
}
```

**Files to modify:**
- `src/typechecker.rs` - Replace mock implementation

### 2. Public Symbol Collection
**Priority: High**

Implement parsing of `.bst` files to collect only public symbols:

- Parse each `.bst` file in a module directory
- Filter for items with `is_public: true`
- Create `SymbolSignature` objects for each public symbol
- Handle different symbol types (functions, structs, enums, etc.)

**Files to modify:**
- `src/typechecker.rs` - Add file parsing logic
- May need new module for file system operations

### 3. Circular Import Detection
**Priority: Medium**

Add cycle detection to prevent circular imports:

```rust
// Example circular import:
// file1.bst: import Self::Module2;
// file2.bst: import Self::Module1;
```

**Implementation:**
- Track import dependencies in a graph
- Detect cycles during import processing
- Show clear error messages with the import cycle

**Files to modify:**
- `src/typechecker.rs` - Add dependency tracking

### 4. Better Error Handling
**Priority: Medium**

Improve error messages for import-related issues:

- Module not found: `"Module 'Std::NonExistent' not found"`
- Symbol not found: `"Symbol 'println' not found in module 'Std::Fmt'"`
- Symbol not public: `"Symbol 'internal_func' is not public in module 'Std::Fmt'"`
- Circular import: `"Circular import detected: Std::A → Std::B → Std::A"`

**Files to modify:**
- `src/typechecker.rs` - Improve error messages

### 5. Module Path Resolution
**Priority: Low**

Implement proper module path resolution:

- Handle nested modules: `Std::Collections::Vec::new`
- Support for module hierarchies
- Proper namespace resolution

### 6. Import Conflict Resolution
**Priority: Low**

Handle import conflicts as discussed:

```rust
// This should be a type error:
import Self::Utils;
import Std::Utils;
// Force user to alias one: import Std::Utils as StdUtils;
```

### 7. Performance Optimizations
**Priority: Low**

- Module symbol caching (already implemented)
- Lazy loading of module symbols
- Incremental type checking

## 📁 Module Structure

Current structure (working):
```
modules/
├── std/
│   ├── fmt/print.bst
│   ├── collections/vec.bst
│   ├── string/string.bst
│   └── math/math.bst
utils/
└── helper.bst
```

## 🧪 Testing

Current test files:
- `test_imports.bst` - Basic import test
- `test_imports_comprehensive.bst` - Multiple import scenarios

**Additional tests needed:**
- Circular import detection
- Error handling for missing modules/symbols
- Import conflicts
- Nested module resolution

## 📝 Notes

- The import system is currently working with mock data
- All the infrastructure is in place for real file loading
- The `pub` keyword system is fully implemented
- Path resolution and symbol caching are working correctly

## 🎯 Next Steps

1. **Start with real file system loading** - This is the most important missing piece
2. **Add circular import detection** - Critical for preventing infinite loops
3. **Improve error messages** - Better developer experience
4. **Add comprehensive tests** - Ensure robustness

The foundation is solid - just need to replace mocks with real implementations! 