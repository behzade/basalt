// Tests that ARC objects are released when they go out of scope.
import Std::Fmt;

Fmt.print_string("--- Scope Test Start ---");

if true {
    let inner_str = "This string is created inside a block.";
    Fmt.print_string(inner_str);
} // inner_str should be released here.

Fmt.print_string("--- Scope Test End ---"); 