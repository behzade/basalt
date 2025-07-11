#include <stdio.h>

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