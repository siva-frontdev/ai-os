//! # Event Bus
//!
//! In-process pub/sub event bus.
//!
//! ## Design decisions
//!
//! * **Dyn-compatible** — [`EventBus`] methods operate on
//!   `&dyn Event` and `Arc<dyn ErasedEventHandler>` so the
//!   trait can be used as a trait object.
//! * **Type-safe helpers** — [`typed_publish`] and
//!   [`typed_subscribe`] provide a generic wrapper for
//!   callers who want compile-time safety.
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;

use crate::error::CoreError;
use crate::utils::Id;

// ── Event trait ───────────────────────────────────────────

pub trait Event: Any + Debug + Send + Sync + 'static {
    fn event_type(&self) -> &'static str;
}

impl dyn Event {
    pub fn downcast_ref<T: Event>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }
}

// ── Handler traits ────────────────────────────────────────

#[async_trait]
pub trait ErasedEventHandler: Debug + Send + Sync {
    async fn handle(&self, event: &dyn Event) -> Result<(), CoreError>;
    fn event_type_id(&self) -> TypeId;
}

#[async_trait]
pub trait EventHandler<E: Event>: Debug + Send + Sync + 'static {
    async fn handle(&self, event: &E) -> Result<(), CoreError>;
}

#[derive(Debug)]
struct HandlerAdapter<E: Event, H: EventHandler<E>> {
    inner: Arc<H>,
    _marker: std::marker::PhantomData<E>,
}

#[async_trait]
impl<E: Event + 'static, H: EventHandler<E> + 'static> ErasedEventHandler
    for HandlerAdapter<E, H>
{
    async fn handle(&self, event: &dyn Event) -> Result<(), CoreError> {
        let downcasted = event.downcast_ref::<E>().ok_or_else(|| {
            CoreError::General(format!(
                "event type mismatch: expected {}",
                std::any::type_name::<E>()
            ))
        })?;
        self.inner.handle(downcasted).await
    }

    fn event_type_id(&self) -> TypeId {
        TypeId::of::<E>()
    }
}

// ── Subscription ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Subscription {
    id: Id,
}

impl Subscription {
    fn new() -> Self {
        Self { id: Id::new() }
    }
}

// ── EventBus trait (dyn-compatible) ───────────────────────

/// Type-erased event bus — all methods operate on `&dyn Event`.
#[async_trait]
pub trait EventBus: Debug + Send + Sync {
    /// Publish an event to all handlers registered for its type.
    async fn publish_event(&self, event: &dyn Event) -> Result<(), CoreError>;

    /// Subscribe an erased handler.  Prefer [`typed_subscribe`]
    /// for type safety.
    fn subscribe_erased(
        &self,
        type_id: TypeId,
        handler: Arc<dyn ErasedEventHandler>,
    ) -> Result<Subscription, CoreError>;

    /// Remove a subscription.
    fn unsubscribe(&self, sub: &Subscription) -> Result<(), CoreError>;
}

/// Type-safe publish helper.
pub async fn typed_publish<E: Event>(
    bus: &dyn EventBus,
    event: &E,
) -> Result<(), CoreError> {
    bus.publish_event(event).await
}

/// Type-safe subscribe helper.
pub fn typed_subscribe<E: Event + 'static, H: EventHandler<E> + 'static>(
    bus: &dyn EventBus,
    handler: Arc<H>,
) -> Result<Subscription, CoreError> {
    let adapter = Arc::new(HandlerAdapter {
        inner: handler,
        _marker: std::marker::PhantomData,
    }) as Arc<dyn ErasedEventHandler>;
    bus.subscribe_erased(TypeId::of::<E>(), adapter)
}

// ── In-memory implementation ──────────────────────────────

#[derive(Debug)]
struct SubscriberEntry {
    sub: Subscription,
    handler: Arc<dyn ErasedEventHandler>,
}

#[derive(Debug)]
pub struct InMemoryEventBus {
    subscribers: RwLock<HashMap<TypeId, Vec<SubscriberEntry>>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish_event(&self, event: &dyn Event) -> Result<(), CoreError> {
        let type_id = (event as &dyn Any).type_id();
        let handlers: Vec<Arc<dyn ErasedEventHandler>> = {
            let guard = self.subscribers.read().map_err(|_| CoreError::LockPoisoned)?;
            match guard.get(&type_id) {
                None => return Ok(()),
                Some(entries) => entries.iter().map(|e| e.handler.clone()).collect(),
            }
        };

        for handler in &handlers {
            handler.handle(event).await.map_err(|e| {
                CoreError::HandlerFailed {
                    event_type: event.event_type(),
                    detail: e.to_string(),
                }
            })?;
        }
        Ok(())
    }

    fn subscribe_erased(
        &self,
        type_id: TypeId,
        handler: Arc<dyn ErasedEventHandler>,
    ) -> Result<Subscription, CoreError> {
        let sub = Subscription::new();
        let entry = SubscriberEntry {
            sub: sub.clone(),
            handler,
        };
        let mut guard = self.subscribers.write().map_err(|_| CoreError::LockPoisoned)?;
        guard.entry(type_id).or_default().push(entry);
        Ok(sub)
    }

    fn unsubscribe(&self, sub: &Subscription) -> Result<(), CoreError> {
        let mut guard = self.subscribers.write().map_err(|_| CoreError::LockPoisoned)?;
        for entries in guard.values_mut() {
            entries.retain(|e| &e.sub != sub);
        }
        Ok(())
    }
}

// ── Convenience events ────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ShutdownEvent;

impl Event for ShutdownEvent {
    fn event_type(&self) -> &'static str {
        "platform.shutdown"
    }
}

#[derive(Debug, Clone)]
pub struct StartedEvent;

impl Event for StartedEvent {
    fn event_type(&self) -> &'static str {
        "platform.started"
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug, Clone)]
    struct TestEvent {
        value: u32,
    }

    impl Event for TestEvent {
        fn event_type(&self) -> &'static str {
            "test.event"
        }
    }

    #[derive(Debug)]
    struct CounterHandler {
        count: AtomicU32,
    }

    #[async_trait]
    impl EventHandler<TestEvent> for CounterHandler {
        async fn handle(&self, event: &TestEvent) -> Result<(), CoreError> {
            self.count.fetch_add(event.value, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn publish_and_receive() {
        let bus = InMemoryEventBus::new();
        let handler = Arc::new(CounterHandler {
            count: AtomicU32::new(0),
        });
        typed_subscribe::<TestEvent, CounterHandler>(&bus, handler.clone()).unwrap();
        typed_publish(&bus, &TestEvent { value: 5 }).await.unwrap();
        assert_eq!(handler.count.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn multiple_subscribers() {
        let bus = InMemoryEventBus::new();
        let h1 = Arc::new(CounterHandler {
            count: AtomicU32::new(0),
        });
        let h2 = Arc::new(CounterHandler {
            count: AtomicU32::new(0),
        });
        typed_subscribe(&bus, h1.clone()).unwrap();
        typed_subscribe(&bus, h2.clone()).unwrap();
        typed_publish(&bus, &TestEvent { value: 10 }).await.unwrap();
        assert_eq!(h1.count.load(Ordering::SeqCst), 10);
        assert_eq!(h2.count.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn no_subscribers_is_ok() {
        let bus = InMemoryEventBus::new();
        typed_publish(&bus, &TestEvent { value: 1 }).await.unwrap();
    }

    #[tokio::test]
    async fn unsubscribe_works() {
        let bus = InMemoryEventBus::new();
        let handler = Arc::new(CounterHandler {
            count: AtomicU32::new(0),
        });
        let sub = typed_subscribe::<TestEvent, CounterHandler>(&bus, handler.clone()).unwrap();
        typed_publish(&bus, &TestEvent { value: 1 }).await.unwrap();
        assert_eq!(handler.count.load(Ordering::SeqCst), 1);

        bus.unsubscribe(&sub).unwrap();
        typed_publish(&bus, &TestEvent { value: 1 }).await.unwrap();
        assert_eq!(handler.count.load(Ordering::SeqCst), 1);
    }
}
