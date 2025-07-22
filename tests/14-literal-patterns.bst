// Test 14: Literal and Wildcard Patterns
// Testing literal patterns, wildcard patterns, and mixed patterns in match expressions

fn simple_test() -> i64 {
    42
}

fn http_status_text(code: i64) -> string {
    match code {
        200 => "OK",
        404 => "Not Found",
        _ => "Unknown Status"
    }
}

// Test with boolean patterns
fn is_valid_status(code: i64) -> bool {
    match code {
        200 => true,
        404 => true,
        _ => false
    }
}

// Test with string patterns
fn get_message(msg: string) -> string {
    match msg {
        "hello" => "Hi there!",
        "goodbye" => "See you later!",
        _ => "Unknown message"
    }
}

// Test mixed patterns with enum
enum Status {
    Success(i64),
    Error(string),
    Unknown,
}

fn process_status(status: Status) -> string {
    match status {
        Success(200) => "Perfect!",
        Success(404) => "Not found error",
        Success(_) => "Other success",
        Error("timeout") => "Timeout error",
        Error(_) => "Other error",
        Unknown => "Unknown status"
    }
} 