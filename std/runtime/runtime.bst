extern fn printf(format: string, ...) -> none;

extern fn strlen(s: string) -> int64;
extern fn strcpy(dest: int64, src: string) -> none;
extern fn strcat(dest: int64, src: string) -> none;
extern fn strcmp(s1: string, s2: string) -> int64;

extern fn malloc(size: int64) -> int64;
extern fn free(ptr: int64) -> none;
