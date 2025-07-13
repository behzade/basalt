// Test array functionality
import Std::Fmt;
Fmt.print_string("Array Tests:");

// Test basic array creation and access
let arr = [1, 2, 3, 4, 5];
Fmt.print_int(arr[0]);
Fmt.print_int(arr[2]);
Fmt.print_int(arr[4]);

// Test array length
// Fmt.print_int(arr.len());

// Test empty array
// let empty = [];
// Fmt.print_int(empty.len());

// Test array with expressions
let computed = [1 + 1, 2 * 3, 10 / 2];
Fmt.print_int(computed[0]);
Fmt.print_int(computed[1]);
Fmt.print_int(computed[2]);

// Test variable indexing
let i = 1;
Fmt.print_int(arr[i]);

// Test complex index expressions
Fmt.print_int(arr[0 + 2]);
Fmt.print_int(arr[4 - 1]);

// Test single element array
let single = [42];
Fmt.print_int(single[0]);
// Fmt.print_int(single.len());

// Test array in arithmetic
let sum = arr[0] + arr[1] + arr[2];
Fmt.print_int(sum);

// Test nested array operations
let first = [10, 20];
let second = [30, 40];
Fmt.print_int(first[0] + second[1]); 
