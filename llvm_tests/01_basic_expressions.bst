// Test basic expressions, variables, and print
import Std::Fmt;

let x = 42;
let y = 10;
let sum = x + y;
Fmt.print_int(sum);

let product = x * y;
Fmt.print_int(product);

let comparison = x > y;
Fmt.print_bool(comparison); 
