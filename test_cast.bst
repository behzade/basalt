let MyStruct = struct { x: int64 }; unsafe { let raw: rawptr = 0; let typed: *MyStruct = raw; }
