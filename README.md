# Pubsub Crate

[![Crates.io](https://img.shields.io/crates/v/pubsub-rs)](https://crates.io/crates/pubsub-rs)
[![Docs.rs](https://docs.rs/pubsub/badge.svg)](https://docs.rs/pubsub-rs)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)

A publish-subscribe system for Rust with async/await support.

This crate provides a simple yet powerful publish-subscribe (pubsub) system that allows multiple
subscribers to receive messages published to specific topics. It is designed to be thread-safe,
async-friendly, memory-efficient, and supports clean shutdown.

## Features

- **Thread-safe**: Uses `Arc` and `DashMap` for concurrent access.
- **Async-friendly**: Built with async/await support using `async-channel`.
- **Memory efficient**: Uses weak references to prevent memory leaks.
- **Clean shutdown**: Automatically cleans up resources when dropped.
- **Multiple subscribers per topic**: Supports multiple subscribers for each topic.
- **Multiple topics per subscriber**: Subscribers can subscribe to multiple topics.
- **Graceful shutdown handling**: Ensures proper cleanup when the system is dropped.

## Usage

Add the following to your `Cargo.toml`:

```toml
[dependencies]
pubsub-rs = "0.1.0"
```

### Basic Example

```rust
use pubsub::{Pubsub, PubsubError};

#[tokio::main]
async fn main() {
    let pubsub = Pubsub::new();

    // Subscribe to topics
    let subscriber = pubsub.subscribe(vec!["topic1", "topic2"]).await;

    // Publish messages
    pubsub.publish("topic1", "Hello".to_owned()).await;

    // Receive messages
    let (topic, message) = subscriber.recv().await.unwrap();
    assert_eq!(topic, "topic1");
    assert_eq!(message, "Hello");
}
```

### Error Handling

The main error type is `PubsubError`, which occurs when:

- The pubsub system has been closed.
- A subscriber tries to receive messages after the pubsub system is dropped.

### Performance Considerations

- Uses `DashMap` for concurrent topic storage.
- Each subscriber has its own async channel.
- Message delivery is non-blocking.
- Automatic cleanup of dropped subscribers.

### Safety

- All operations are thread-safe.
- Uses `Arc` for shared ownership.
- Uses `Weak` references to prevent memory leaks.
- Proper cleanup on drop.

## Examples

See the [tests module](https://github.com/alaingilbert/pubsub-rs/blob/master/src/tests.rs) for more comprehensive usage examples.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE.txt) file for details.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request on [GitHub](https://github.com/alaingilbert/pubsub-rs).

## Acknowledgements

- `async-channel` for providing the async channels used in this crate.
- `DashMap` for providing the concurrent hash map used for topic storage.
