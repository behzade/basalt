// Hash Map Tests
import Std::Fmt;
import Std::Collections;

Fmt.print_string("--- Hash Map Tests ---");

// Test 1: Literal, Set, and Get
// The literal syntax will now be compiled into new() + set() calls.
let mut scores = {"math": 95, "history": 88};
Collections::set(scores, "english", 92); // Use function call syntax

Fmt.print_int(Collections::get(scores, "math"));      // Expected: 95
Fmt.print_int(Collections::get(scores, "history"));  // Expected: 88
Fmt.print_int(Collections::get(scores, "english"));  // Expected: 92

// Test 2: Integer keys
// For now, we'll use the basic new() function without generic parameters
let mut user_names = Collections::new();
Collections::set(user_names, 101, "Alice");
Collections::set(user_names, 102, "Bob");

Fmt.print_string(Collections::get(user_names, 101)); // Expected: Alice
Fmt.print_string(Collections::get(user_names, 102)); // Expected: Bob

// Test 3: Empty HashMap
let mut empty_map = Collections::new();
Fmt.print_int(Collections::size(empty_map)); // Expected: 0

// Test 4: HashMap size
Fmt.print_int(Collections::size(scores)); // Expected: 3

// Test 5: Contains check
if Collections::contains(scores, "math") {
    Fmt.print_string("Found math score");
}

if !Collections::contains(scores, "physics") {
    Fmt.print_string("Physics score not found");
}

// Test 6: Index syntax (once compiler supports it)
// scores["physics"] = 92;
// let physics_grade = scores["physics"];
// Fmt.print_int(physics_grade); // Expected: 92

Fmt.print_string("--- Hash Map Tests Complete ---"); 