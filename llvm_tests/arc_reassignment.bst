// Tests that old values are released during variable reassignment.
import Std::Fmt;

Fmt.print_string("--- Reassignment Test ---");

let mut s = "first";
Fmt.print_string(s);

s = "second"; // "first" should be released here.
Fmt.print_string(s);

s = "third"; // "second" should be released here.
Fmt.print_string(s); 