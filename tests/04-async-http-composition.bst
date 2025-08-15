effect Http {
    get(url: str) -> str
}

effect Async<T> {
    await(task: () -> T) -> T
}

handler Scheduler: Async<T> {
    fn await(task: () -> T) -> T {
        let res = task()
        res
    }
}

handler HttpHandler: Http with Async<str> {
    fn get(url: str) -> str with {Async<str>} {
        fn http_task() -> str = { "<html>" }
        let content = perform Async.await(http_task)
        content
    }
}

let AsyncHttpHandler = HttpHandler with {Scheduler}

fn fetch_content_size(url: str) -> i32 with {Http} {
    let content = perform Http.get(url)
    content.len()
}

fn main() -> i32 {
    let size = fetch_content_size("https://example.com") with {AsyncHttpHandler}
    size
}
