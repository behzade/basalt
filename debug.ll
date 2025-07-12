@str_7 = private unnamed_addr global [17 x i8] [i8 68, i8 111, i8 110, i8 101, i8 32, i8 119, i8 105, i8 116, i8 104, i8 32, i8 108, i8 111, i8 111, i8 112, i8 115, i8 33, i8 0]

define i32 @main() {
0:
	%1 = alloca i64
	store i64 10, i64* %1
	%2 = alloca i64
	store i64 5, i64* %2
	%3 = load i64, i64* %1
	%4 = load i64, i64* %2
	%5 = icmp sgt i64 %3, %4
	br i1 %5, label %if.then.0, label %if.else.0

if.then.0:
	%6 = load i64, i64* %1
	br label %if.merge.0

if.else.0:
	%7 = load i64, i64* %2
	br label %if.merge.0

if.merge.0:
	%8 = phi i64 [ %6, %if.then.0 ], [ %7, %if.else.0 ]
	%9 = alloca i64
	store i64 %8, i64* %9
	%10 = load i64, i64* %9
	call void @basalt_print_int(i64 %10)
	%11 = load i64, i64* %1
	%12 = load i64, i64* %2
	%13 = icmp slt i64 %11, %12
	br i1 %13, label %if.then.1, label %if.else.1

if.then.1:
	%14 = load i64, i64* %1
	br label %if.merge.1

if.else.1:
	%15 = load i64, i64* %2
	br label %if.merge.1

if.merge.1:
	%16 = phi i64 [ %14, %if.then.1 ], [ %15, %if.else.1 ]
	%17 = alloca i64
	store i64 %16, i64* %17
	%18 = load i64, i64* %17
	call void @basalt_print_int(i64 %18)
	%19 = alloca i64
	store i64 0, i64* %19
	%20 = alloca i64
	store i64 3, i64* %20
	br label %loop.cond.2

loop.cond.2:
	%21 = load i64, i64* %19
	%22 = load i64, i64* %20
	%23 = icmp slt i64 %21, %22
	br i1 %23, label %loop.body.2, label %loop.exit.2

loop.body.2:
	%24 = load i64, i64* %19
	call void @basalt_print_int(i64 %24)
	%25 = load i64, i64* %19
	%26 = load i64, i64* %19
	%27 = add i64 %26, 1
	store i64 %27, i64* %19
	br label %loop.cond.2

loop.exit.2:
	%28 = alloca i64
	store i64 10, i64* %28
	br label %loop.cond.3

loop.cond.3:
	%29 = load i64, i64* %28
	%30 = icmp sgt i64 %29, 5
	br i1 %30, label %loop.body.3, label %loop.exit.3

loop.body.3:
	%31 = load i64, i64* %28
	call void @basalt_print_int(i64 %31)
	%32 = load i64, i64* %28
	%33 = load i64, i64* %28
	%34 = sub i64 %33, 2
	store i64 %34, i64* %28
	br label %loop.cond.3

loop.exit.3:
	%35 = alloca i64
	store i64 0, i64* %35
	br label %loop.cond.4

loop.cond.4:
	%36 = load i64, i64* %35
	%37 = icmp sgt i64 %36, 10
	br i1 %37, label %loop.body.4, label %loop.exit.4

loop.body.4:
	%38 = load i64, i64* %35
	call void @basalt_print_int(i64 %38)
	%39 = load i64, i64* %35
	%40 = load i64, i64* %35
	%41 = add i64 %40, 1
	store i64 %41, i64* %35
	br label %loop.cond.4

loop.exit.4:
	%42 = alloca i64
	store i64 0, i64* %42
	br label %loop.cond.5

loop.cond.5:
	%43 = load i64, i64* %42
	%44 = icmp slt i64 %43, 2
	br i1 %44, label %loop.body.5, label %loop.exit.5

loop.body.5:
	%45 = alloca i64
	store i64 0, i64* %45
	br label %loop.cond.6

loop.exit.5:
	call void @basalt_print_string(i8* getelementptr ([17 x i8], [17 x i8]* @str_7, i64 0, i64 0))
	ret i32 0

loop.cond.6:
	%46 = load i64, i64* %45
	%47 = icmp slt i64 %46, 2
	br i1 %47, label %loop.body.6, label %loop.exit.6

loop.body.6:
	%48 = load i64, i64* %42
	call void @basalt_print_int(i64 %48)
	%49 = load i64, i64* %45
	call void @basalt_print_int(i64 %49)
	%50 = load i64, i64* %45
	%51 = load i64, i64* %45
	%52 = add i64 %51, 1
	store i64 %52, i64* %45
	br label %loop.cond.6

loop.exit.6:
	%53 = load i64, i64* %42
	%54 = load i64, i64* %42
	%55 = add i64 %54, 1
	store i64 %55, i64* %42
	br label %loop.cond.5
}

declare void @basalt_print_int(i64 %val)

declare void @basalt_print_bool(i1 %val)

declare void @basalt_print_string(i8* %str)

declare void @basalt_print_float(double %val)

declare i8* @basalt_string_concat(i8* %s1, i8* %s2)

declare i1 @basalt_string_equals(i8* %s1, i8* %s2)

declare i8* @basalt_array_new(i64 %capacity)

declare void @basalt_array_push(i8* %arr, i64 %value)

declare i64 @basalt_array_get(i8* %arr, i64 %index)

declare i64 @basalt_array_len(i8* %arr)
