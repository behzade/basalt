// Test enum types and pattern matching

// Define an Option enum
let Option = enum {
    Some(int64),
    None,
}

// Define a NetEvent enum  
let NetEvent = enum {
    Connect,
    Disconnect,
    Message(string),
}

// Test Option enum
let some_val = Option::Some(100);
let no_val = Option::None;

// Test pattern matching with Option
let my_option = Option::Some(41);

let result = match my_option {
    Option::Some(x) => x + 1,
    Option::None => 0,
};

print(result); // Should print 42

// Test NetEvent enum
let msg = NetEvent::Message("hello");
let connect_event = NetEvent::Connect;

// Test pattern matching with NetEvent
let event_result = match msg {
    NetEvent::Connect => 1,
    NetEvent::Disconnect => 2,
    NetEvent::Message(s) => 3,
};

print(event_result); // Should print 3

// Test with None variant
let none_result = match no_val {
    Option::Some(x) => x,
    Option::None => -1,
};

print(none_result); // Should print -1 
