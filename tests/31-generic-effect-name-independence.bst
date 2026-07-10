// Generic effect inference must not depend on an effect or operation's spelling.
effect Deferred<T> {
    wait(task: () -> T) -> T
}

handler Runner: Deferred<T> {
    fn wait(task: () -> T) -> T {
        task()
    }
}

fn main() -> i32 {
    let task = fn() -> i32 { 41 }
    perform Deferred.wait(task) with {Runner}
}
