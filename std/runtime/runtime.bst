extern fn printf(format: string, ...) -> int64;

extern fn strlen(s: string) -> int64;
extern fn strcpy(dest: int64, src: string) -> none;
extern fn strcat(dest: int64, src: string) -> none;
extern fn strcmp(s1: string, s2: string) -> int64;

extern fn GC_malloc(size: int64) -> rawptr;

extern fn malloc(size: int64) -> rawptr;
extern fn free(ptr: rawptr) -> none;
