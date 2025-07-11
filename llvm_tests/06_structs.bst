// Struct Tests - Testing struct definitions, instantiation, and field access

print("Struct Tests:");

// Test 1: Basic struct definition and instantiation
let Point = struct { x: int64, y: int64 };
let p1 = Point { x: 10, y: 20 };
print(p1.x);
print(p1.y);

// Test 2: Multiple field types
let Person = struct { name: string, age: int64, active: bool };
let john = Person { name: "John Doe", age: 30, active: true };
print(john.name);
print(john.age);
print(john.active);

// Test 3: Struct field access in expressions
let p2 = Point { x: 5, y: 15 };
print(p1.x + p2.x);
print(p1.y + p2.y);

// Test 4: Struct fields in arithmetic and boolean operations
let Rectangle = struct { width: int64, height: int64 };
let rect = Rectangle { width: 8, height: 6 };
print(rect.width * rect.height);
print(rect.width > rect.height);

// Test 5: String field operations
let Config = struct { name: string, enabled: bool };
let config = Config { name: "test", enabled: true };
print(config.name);
print(config.enabled);

// Test 6: Complex expressions with struct fields
let Numbers = struct { a: int64, b: int64, c: int64 };
let nums = Numbers { a: 2, b: 3, c: 4 };
print(nums.a + nums.b * nums.c);
print(nums.a * nums.b + nums.c);

// Test 7: Multiple struct instances
let p3 = Point { x: 1, y: 2 };
let p4 = Point { x: 3, y: 4 };
print(p3.x + p4.x);
print(p3.y + p4.y);

// Test 8: Boolean field operations
let Flags = struct { debug: bool, verbose: bool };
let flags = Flags { debug: true, verbose: false };
print(flags.debug);
print(flags.verbose); 