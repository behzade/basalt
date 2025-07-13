extern fn printf(format: string, ...) -> none;

extern fn strlen(s: string) -> int64;
extern fn strcpy(dest: array_ptr, src: string) -> none;
extern fn strcat(dest: array_ptr, src: string) -> none;
extern fn strcmp(s1: string, s2: string) -> int64;

extern fn malloc(size: int64) -> array_ptr;
extern fn free(ptr: array_ptr) -> none;
