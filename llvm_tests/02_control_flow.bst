// Test if-else control flow
let x = 10;
let y = 5;

let max = if x > y {
    x
} else {
    y
};
print(max);

let min = if x < y {
    x
} else {
    y
};
print(min);

// Test for loops
let mut counter = 0;
let limit = 3;

// Simple counting loop
for counter < limit {
    print(counter);
    counter = counter + 1;
}

// Test loop with different condition
let mut i = 10;
for i > 5 {
    print(i);
    i = i - 2;
}

// Test loop that doesn't execute
let mut j = 0;
for j > 10 {
    print(j);
    j = j + 1;
}

// Test nested loops
let mut outer = 0;
for outer < 2 {
    let mut inner = 0;
    for inner < 2 {
        print(outer);
        print(inner);
        inner = inner + 1;
    }
    outer = outer + 1;
}

print("Done with loops!"); 