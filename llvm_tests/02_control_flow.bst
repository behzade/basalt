// Test if-else control flow
import Std::Fmt;
let x = 10;
let y = 5;

let max = if x > y {
    x
} else {
    y
};
Fmt.print_int(max);

let min = if x < y {
    x
} else {
    y
};
Fmt.print_int(min);

// Test for loops
let mut counter = 0;
let limit = 3;

// Simple counting loop
for counter < limit {
    Fmt.print_int(counter);
    counter = counter + 1;
}

// Test loop with different condition
let mut i = 10;
for i > 5 {
    Fmt.print_int(i);
    i = i - 2;
}

// Test loop that doesn't execute
let mut j = 0;
for j > 10 {
    Fmt.print_int(j);
    j = j + 1;
}

// Test nested loops
let mut outer = 0;
for outer < 2 {
    let mut inner = 0;
    for inner < 2 {
        Fmt.print_int(outer);
        Fmt.print_int(inner);
        inner = inner + 1;
    }
    outer = outer + 1;
}

Fmt.print_string("Done with loops!"); 
