#include <stdio.h>
#include <stdlib.h> // For malloc, realloc
#include <string.h>

// Print an integer value followed by a newline
void basalt_print_int(long long val) {
    printf("%lld\n", val);
}

// Print a boolean value followed by a newline
void basalt_print_bool(int val) {
    printf("%s\n", val ? "true" : "false");
}

// Print a string value followed by a newline
void basalt_print_string(const char* str) {
    printf("%s\n", str);
}

// Print a float value followed by a newline
void basalt_print_float(double val) {
    printf("%f\n", val);
}

// BasaltArray struct for dynamically-sized arrays of 64-bit integers
typedef struct {
    long long length;
    long long capacity;
    long long* data;
} BasaltArray;

// Creates a new array with an initial capacity.
BasaltArray* basalt_array_new(long long initial_capacity) {
    BasaltArray* arr = malloc(sizeof(BasaltArray));
    arr->length = 0;
    arr->capacity = initial_capacity > 0 ? initial_capacity : 8; // Default capacity
    arr->data = malloc(arr->capacity * sizeof(long long));
    return arr;
}

// Appends an element to the array, resizing if necessary.
void basalt_array_push(BasaltArray* arr, long long value) {
    if (arr->length >= arr->capacity) {
        arr->capacity *= 2;
        arr->data = realloc(arr->data, arr->capacity * sizeof(long long));
    }
    arr->data[arr->length] = value;
    arr->length++;
}

// Gets an element at a specific index. (No bounds checking for now).
long long basalt_array_get(BasaltArray* arr, long long index) {
    // Add bounds checking later if desired.
    return arr->data[index];
}

// Gets the current length of the array.
long long basalt_array_len(BasaltArray* arr) {
    return arr->length;
} 