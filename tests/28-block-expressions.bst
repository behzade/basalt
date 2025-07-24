// Test 28: Block expressions
// Testing block expressions that evaluate to their last expression

fn main() -> i32 {
    let a = {
        2
    };
    
    let b = {
        let x = 10;
        let y = 20;
        x + y
    };
    
    let c = {
        if true {
            42
        } else {
            100
        }
    };
    
    return 0;
} 