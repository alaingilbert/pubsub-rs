//! A publish-subscribe system for Rust with async/await support.
//!
//! This crate provides a simple yet powerful publish-subscribe (pubsub) system
//! that allows multiple subscribers to receive messages published to specific topics.
//! It's designed to be:
//! - Thread-safe: Uses Arc and DashMap for concurrent access
//! - Async-friendly: Built with async/await support using async-channel
//! - Memory efficient: Uses weak references to prevent memory leaks
//! - Clean shutdown: Automatically cleans up resources when dropped
//!
//! # Features
//! - Multiple subscribers per topic
//! - Multiple topics per subscriber
//! - Thread-safe operations
//! - Async message delivery
//! - Automatic cleanup of dropped subscribers
//! - Graceful shutdown handling
//!
//! # Basic Usage
//! ```rust
//! use pubsub_rs::{Pubsub, PubsubError};
//!
//! #[tokio::main]
//! async fn main() {
//!     let pubsub = Pubsub::new();
//!
//!     // Subscribe to topics
//!     let subscriber = pubsub.subscribe(vec!["topic1", "topic2"]).await;
//!
//!     // Publish messages
//!     pubsub.publish("topic1", "Hello".to_owned()).await;
//!
//!     // Receive messages
//!     let (topic, message) = subscriber.recv().await.unwrap();
//!     assert_eq!(topic, "topic1");
//!     assert_eq!(message, "Hello");
//! }
//! ```
//!
//! # Error Handling
//! The main error type is `PubsubError`, which occurs when:
//! - The pubsub system has been closed
//! - A subscriber tries to receive messages after the pubsub system is dropped
//!
//! # Performance Considerations
//! - Uses DashMap for concurrent topic storage
//! - Each subscriber has its own async channel
//! - Message delivery is non-blocking
//! - Automatic cleanup of dropped subscribers
//!
//! # Safety
//! - All operations are thread-safe
//! - Uses Arc for shared ownership
//! - Uses Weak references to prevent memory leaks
//! - Proper cleanup on drop
//!
//! # Examples
//! See the tests module for more comprehensive usage examples.
use std::error::Error;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::{Arc, Weak};
use async_channel::{unbounded, Receiver, Sender};
use dashmap::DashMap;

#[cfg(test)]
mod tests;

/// A trait that defines the requirements for types that can be used as Pubsub topics.
///
/// Any type that implements Clone, Hash, and Eq can be used as a topic in the Pubsub system.
/// This includes common types like String, &str, and custom types that implement these traits.
///
/// The trait is automatically implemented for all types that satisfy the trait bounds,
/// so users don't need to explicitly implement it for their types.
///
/// # Requirements
/// - Clone: Topics need to be cloned when creating subscriptions and publishing messages
/// - Hash: Topics are used as keys in a hash map for efficient lookups
/// - Eq: Topics need to be comparable for equality when matching subscriptions
///
/// # Examples
/// ```rust
/// // String implements PubsubTopic
/// let topic: String = "my_topic".to_owned();
///
/// // &str implements PubsubTopic
/// let topic: &str = "my_topic";
///
/// // Custom types can be used if they implement the required traits
/// #[derive(Clone, Hash, Eq, PartialEq)]
/// struct CustomTopic {
///     id: u32,
///     name: String,
/// }
/// ```
pub trait PubsubTopic: Clone + Hash + Eq {}
impl<T: Clone + Hash + Eq> PubsubTopic for T {}

/// A publish-subscribe system that allows multiple subscribers to receive messages
/// published to specific topics.
///
/// The Pubsub struct is the main interface for creating topics, publishing messages,
/// and managing subscriptions. It uses an internal Arc reference to shared state,
/// allowing multiple clones of the Pubsub instance to share the same underlying
/// subscription data.
///
/// # Type Parameters
/// * `T` - The topic type, must implement PubsubTopic (Clone + Hash + Eq)
/// * `P` - The payload type, must implement Clone
///
/// # Examples
/// ```rust
/// // let pubsub = Pubsub::new();
/// // let subscriber = pubsub.subscribe(vec!["topic1", "topic2"]).await;
/// // pubsub.publish("topic1", "Hello, world!".to_owned()).await;
/// ```
#[derive(Clone)]
pub struct Pubsub<T: PubsubTopic, P: Clone> {
    inner: Arc<PubsubInner<T, P>>,
}

/// Implements the Drop trait for Pubsub to ensure proper cleanup when the last instance is dropped.
///
/// When the last strong reference to the Pubsub's inner data is dropped, this implementation:
/// 1. Checks if this is the last strong reference (Arc::strong_count == 1)
/// 2. If so, iterates through all topics and their subscribers
/// 3. Closes each subscriber's channel to prevent them from being stuck waiting for messages
///
/// This ensures that any remaining subscribers will receive an error on their next recv() call
/// rather than blocking indefinitely, allowing them to clean up their resources properly.
impl<T: PubsubTopic, P: Clone> Drop for Pubsub<T, P> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            for subs in self.inner.m.iter() {
                for sub_inner in subs.iter() {
                    sub_inner.tx.close();
                }
            }
        }
    }
}

impl<T: PubsubTopic, P: Clone> Pubsub<T, P> {
    /// Creates a new Pubsub instance with empty topic subscriptions.
    ///
    /// This initializes the internal shared state and returns a new Pubsub instance
    /// that can be used to manage topics and subscriptions.
    ///
    /// # Examples
    /// ```rust
    /// // let pubsub = Pubsub::new();
    /// ```
    pub fn new() -> Self {
        Self { inner: Arc::new(PubsubInner::new()) }
    }

    /// Subscribes to one or more topics and returns a new Subscriber instance.
    ///
    /// # Arguments
    /// * `topics` - A vector of topics to subscribe to. The subscriber will receive messages
    ///             published to any of these topics.
    ///
    /// # Returns
    /// A new `Subscriber` instance that can be used to receive messages for the subscribed topics.
    ///
    /// # Example
    /// ```rust
    /// // let pubsub = Pubsub::new();
    /// // let subscriber = pubsub.subscribe(vec!["topic1", "topic2"]).await;
    /// ```
    pub async fn subscribe(&self, topics: Vec<T>) -> Subscriber<T, P> {
        let w = Arc::downgrade(&self.inner);
        let sub = Subscriber::new(w, topics);
        self.inner.add_subscriber(&Arc::clone(&sub.inner));
        sub
    }

    /// Publishes a message to a specific topic.
    ///
    /// # Arguments
    /// * `topic` - The topic to publish the message to. Subscribers subscribed to this topic
    ///            will receive the message.
    /// * `payload` - The message payload to send to subscribers.
    ///
    /// # Example
    /// ```rust
    /// // let pubsub = Pubsub::new();
    /// // pubsub.publish("topic1", "Hello, world!".to_owned()).await;
    /// ```
    pub async fn publish(&self, topic: T, payload: P) {
        self.inner.publish(topic, payload).await;
    }
}

struct PubsubInner<T: PubsubTopic, P: Clone> {
    m: DashMap<T, Vec<Arc<SubscriberInner<T, P>>>>,
}

impl<T: PubsubTopic, P: Clone> PubsubInner<T, P> {
    fn new() -> Self {
        Self { m: DashMap::new() }
    }

    async fn publish(&self, topic: T, payload: P) {
        if let Some(subs) = self.m.get(&topic) {
            for sub in subs.iter() {
                sub.publish(Payload::new(topic.clone(), payload.clone())).await;
            }
        }
    }

    fn add_subscriber(&self, sub: &Arc<SubscriberInner<T, P>>) {
        for topic in &sub.topics {
            self.m.entry(topic.clone()).or_insert_with(Vec::new).push(Arc::clone(&sub));
        }
    }

    fn remove_subscriber(&self, sub: &Arc<SubscriberInner<T, P>>) {
        for topic in &sub.topics {
            if let Some(mut subs) = self.m.get_mut(topic) {
                subs.retain(|other| !Arc::ptr_eq(sub, other));
            }
        }
    }
}

struct Payload<T, P> {
    topic: T,
    payload: P,
}

impl<T, P> Payload<T, P> {
    fn new(topic: T, payload: P) -> Self {
        Self { topic, payload }
    }
}

/// A subscriber that receives messages for subscribed topics from a Pubsub system.
///
/// The Subscriber struct represents an active subscription to one or more topics.
/// It contains an internal Arc reference to shared state that manages the message
/// channel and subscription information.
///
/// # Type Parameters
/// * `T` - The topic type, must implement PubsubTopic (Clone + Hash + Eq)
/// * `P` - The payload type, must implement Clone
///
/// # Examples
/// ```rust
/// use pubsub_rs::Pubsub;
/// async fn some_fn() {
///     let pubsub: Pubsub<&str, String> = Pubsub::new();
///     let subscriber = pubsub.subscribe(vec!["topic1", "topic2"]).await;
///     let (topic, message) = subscriber.recv().await.unwrap();
/// }
/// ```
#[derive(Clone)]
pub struct Subscriber<T: PubsubTopic, P: Clone> {
    inner: Arc<SubscriberInner<T, P>>,
}

impl<T: PubsubTopic, P: Clone> Subscriber<T, P> {
    fn new(p: Weak<PubsubInner<T, P>>, topics: Vec<T>) -> Self {
        let inner = Arc::new(SubscriberInner::new(p, topics));
        Self { inner }
    }

    /// Receives the next message for this subscriber.
    ///
    /// This async function waits until a message is published to one of the subscriber's
    /// subscribed topics, then returns a tuple containing:
    /// 1. The topic the message was published to
    /// 2. The message payload
    ///
    /// # Returns
    /// * `Ok((T, P))` - A tuple containing the topic and payload if a message is received
    /// * `Err(PubsubError)` - If the pubsub system has been closed and no more messages will be sent
    ///
    /// # Examples
    /// ```rust
    /// // let pubsub = Pubsub::new();
    /// // let subscriber = pubsub.subscribe(vec!["topic1"]).await;
    /// // pubsub.publish("topic1", "Hello".to_owned()).await;
    /// // let (topic, message) = subscriber.recv().await.unwrap();
    /// // assert_eq!(topic, "topic1");
    /// // assert_eq!(message, "Hello");
    /// ```
    pub async fn recv(&self) -> Result<(T, P)> {
        self.inner.recv().await
    }
}

/// When Subscriber is dropped, remove itself from all the Pubsub subscriptions
impl<T: PubsubTopic, P: Clone> Drop for Subscriber<T, P> {
    fn drop(&mut self) {
        if let Some(p) = self.inner.p.upgrade() {
            p.remove_subscriber(&self.inner);
        }
    }
}

struct SubscriberInner<T: PubsubTopic, P: Clone> {
    topics: Vec<T>,
    tx: Sender<Payload<T, P>>,
    rx: Receiver<Payload<T, P>>,
    p: Weak<PubsubInner<T, P>>,
}

impl<T: PubsubTopic, P: Clone> SubscriberInner<T, P> {
    fn new(p: Weak<PubsubInner<T, P>>, topics: Vec<T>) -> Self {
        let (tx, rx) = unbounded();
        Self { topics, tx, rx, p }
    }

    async fn recv(&self) -> Result<(T, P)> {
        let Ok(payload) = self.rx.recv().await else {
            return Err(PubsubError)
        };
        Ok((payload.topic, payload.payload))
    }

    async fn publish(&self, payload: Payload<T, P>) {
        let _ = self.tx.send(payload).await;
    }
}

/// Error type returned when a Pubsub operation fails.
///
/// This error occurs when:
/// - The Pubsub system has been closed and no more messages can be received
/// - A subscriber attempts to receive a message after the Pubsub system has been dropped
///
/// # Examples
/// ```rust
/// // let pubsub = Pubsub::new();
/// // let subscriber = pubsub.subscribe(vec!["topic1"]).await;
/// // drop(pubsub);
/// // let result = subscriber.recv().await;
/// // assert!(matches!(result, Err(PubsubError)));
/// ```
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct PubsubError;

impl Display for PubsubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pubsub closed")
    }
}

impl Error for PubsubError {}

type Result<T> = std::result::Result<T, PubsubError>;
