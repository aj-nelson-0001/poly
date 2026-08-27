# Async/Await Now Available in Poly Language

**Published:** August 9, 2026  
**Author:** Poly Language Team  
**Tags:** #poly #async #await #rust #programming

---

## Introduction

We're excited to announce that **async/await support** is now available in the Poly programming language! This major feature brings modern asynchronous programming capabilities to Poly while maintaining its signature explicit, readable syntax.

## What's New

### Async Functions

Declare asynchronous functions using the `async fn` keyword:

~~~poly fragment
async fn fetch_data(url: ustring): Result<ustring, ustring>
    var response := http_get(url).await
    return response
end fn
~~~

This transpiles to idiomatic Rust async code:

~~~rust
async fn fetch_data(url: String) -> Result<String, String> {
    let response = http_get(url).await;
    return Ok(response);
}
~~~

### Await Expressions

Use the `.await` postfix syntax to wait for async operations:

~~~poly fragment
var data := fetch_data(unicode "https://api.example.com").await
~~~

The postfix notation makes the code read naturally: "fetch data,,await the result."

### Traits with Async Methods

Traits can now have async method signatures:

~~~poly fragment
trait DataFetcher
    async fn fetch(self, key: ustring): Result<ustring, ustring>
end trait

impl DataFetcher for HttpClient
    async fn fetch(self, key: ustring): Result<ustring, ustring>
        var response := self.http_get(key).await
        return Ok(response)
    end fn
end impl
~~~

## Complete Example

Here's a complete example demonstrating the new features:

~~~poly fragment
# Define an async trait
trait DataFetcher
    async fn fetch(self, url: ustring): Result<ustring, ustring>
end trait

# Implement the trait with async methods
struct HttpClient
    var base_url: ustring
end struct

impl DataFetcher for HttpClient
    async fn fetch(self, url: ustring): Result<ustring, ustring>
        var full_url := self.base_url + url
        var response := http_get(full_url).await
        
        match response
            Ok(data), return Ok(data)
            Error(e), return Error(unicode "HTTP Error: " + e)
        end match
    end fn
end impl

# Use the async trait
async fn process_data<T: DataFetcher>(fetcher: T, url: ustring): ustring
    match fetcher.fetch(url).await
        Ok(data), return data
        Error(e),
            error e
            return unicode ""
    end match
end fn
~~~

## How It Transpiles

Poly's async/await syntax maps directly to Rust's async system:

| Poly | Rust |
|------|------|
| `async fn name()` | `async fn name()` |
| `expr.await` | `expr.await` |
| `trait T { async fn m(); }` | `trait T { async fn m(); }` |
| `impl T for X { async fn m() {} }` | `impl T for X { async fn m() {} }` |

The transpiler automatically adds `#[tokio::main]` when async functions are detected, so you don't need to manually configure the runtime.

## Why Postfix `.await`?

We chose postfix `.await` syntax for several reasons:

1. **Natural reading order**: "expression.await" reads as "evaluate expression,,await"
2. **Consistency**: Matches method call syntax (expression.method())
3. **Composability**: Easy to chain: `fetch().await.process().await`
4. **Visual clarity**: The dot makes it clear this is an operation on the expression

## What's Next?

With async/await support, we're planning to add:

- **Async iterators** (async for loops)
- **Channels** for inter-task communication
- **Select expressions** for handling multiple concurrent operations
- **Task spawning** primitives

## Getting Started

Update your Poly compiler to the latest version and try the new features:

~~~bash
cd compiler
cargo build --release

# Try the async example
./target/release/poly ../examples/async_await.poly
~~~

## Conclusion

Async/await in Poly brings the power of asynchronous programming while maintaining the language's core philosophy of explicitness and readability. The postfix `.await` syntax and clean trait integration make it easy to write concurrent code that's both powerful and understandable.

Happy coding! 🚀

---

*Have questions or feedback? Open an issue on GitHub or join our Discord community.*
