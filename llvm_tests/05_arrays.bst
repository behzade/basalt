// Test array functionality
print("Array Tests:");

// Test basic array creation and access
let arr = [1, 2, 3, 4, 5];
print(arr[0]);
print(arr[2]);
print(arr[4]);

// Test array length
print(arr.len());

// Test empty array
let empty = [];
print(empty.len());

// Test array with expressions
let computed = [1 + 1, 2 * 3, 10 / 2];
print(computed[0]);
print(computed[1]);
print(computed[2]);

// Test variable indexing
let i = 1;
print(arr[i]);

// Test complex index expressions
print(arr[0 + 2]);
print(arr[4 - 1]);

// Test single element array
let single = [42];
print(single[0]);
print(single.len());

// Test array in arithmetic
let sum = arr[0] + arr[1] + arr[2];
print(sum);

// Test nested array operations
let first = [10, 20];
let second = [30, 40];
print(first[0] + second[1]); 