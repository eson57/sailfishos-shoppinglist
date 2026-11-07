//! Simple debouncer actor.
//!
//! Messages can be enqueued at any time. If the actor is not in a debouncing
//! state then the message is sent immediately, otherwise it is queued up.
//!
//! When queueing messages, the Eq implementation is used to keep only
//! the latest of each equal message.
//!
//! When the debounce timer lapses, all messages are passed to the callback.

use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    marker::PhantomData,
    time::{Duration, Instant},
};

use n0_error::{Result, StdResultExt as _};
use ractor::{ActorProcessingErr, ActorRef};

#[derive(Debug, PartialEq, Eq)]
struct Event<T> {
    item: T,
    release_at: Instant,
}

impl<T: Eq> PartialOrd for Event<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Eq> Ord for Event<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.release_at.cmp(&self.release_at) // reverse ordering for min-heap
    }
}

struct EventBuffer<T> {
    delay: Duration,
    last_triggered: Instant,
    events: BinaryHeap<Event<T>>,
}

enum EventBufferState<T> {
    Ready(T),
    Wait(Duration),
    Empty,
}

impl<T: Eq> EventBuffer<T> {
    fn new(delay: Duration) -> Self {
        Self {
            delay,
            last_triggered: Instant::now(),
            events: BinaryHeap::new(),
        }
    }

    pub fn put(&mut self, item: T) {
        let now = Instant::now();

        // If a matching event was already in the queue, remove it.
        self.events
            .retain(|e| e.release_at <= now || e.item != item);

        // if the last time we emitted something was more than 'delay' ago, we
        // can also just release it immediately.
        if now.duration_since(self.last_triggered) > self.delay {
            self.events.push(Event {
                item,
                release_at: now,
            });

            // set last_triggered here now as we'll be triggering immediately.
            self.last_triggered = now;
        } else {
            self.events.push(Event {
                item,
                release_at: now + self.delay,
            })
        }
    }

    pub fn get(&mut self) -> EventBufferState<T> {
        let now = Instant::now();

        match self.events.peek() {
            None => EventBufferState::Empty,
            Some(e) if e.release_at > now => EventBufferState::Wait(e.release_at - now),
            Some(_) => {
                self.last_triggered = now;
                EventBufferState::Ready(self.events.pop().unwrap().item)
            }
        }
    }
}

pub struct DebouncerActor<T: Eq + Send + 'static>(PhantomData<fn() -> T>);

impl<T: Eq + Send + 'static> DebouncerActor<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Eq + Send + 'static> Default for DebouncerActor<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub enum DebouncerMsg<T: Eq + Send + 'static> {
    Enqueue(T),
    Kick(()),
}

pub struct DebouncerArgs<T: Eq + Send + 'static> {
    pub callback: Box<dyn FnMut(T) -> Result<()> + Send>,
}

pub struct DebouncerState<T: Eq + Send + 'static> {
    buffer: EventBuffer<T>,
    args: DebouncerArgs<T>,
}

#[ractor::actor(
    message = DebouncerMsg<T>,
    state = DebouncerState<T>,
    arguments = DebouncerArgs<T>,
)]
impl<T: Eq + Send + 'static> DebouncerActor<T> {
    async fn pre_start(
        &self,
        _myself: ActorRef<DebouncerMsg<T>>,
        args: DebouncerArgs<T>,
    ) -> Result<DebouncerState<T>, ActorProcessingErr> {
        Ok(DebouncerState {
            buffer: EventBuffer::new(Duration::from_millis(10)),
            args,
        })
    }

    #[ractor::message(DebouncerMsg::Kick(_))]
    fn kick(&self, myself: ActorRef<DebouncerMsg<T>>, state: &mut DebouncerState<T>) -> Result<()> {
        loop {
            match state.buffer.get() {
                EventBufferState::Ready(ready) => {
                    (state.args.callback)(ready)?;
                }
                EventBufferState::Wait(duration) => {
                    ractor::time::send_after(duration, myself.into(), || {
                        DebouncerMsg::<T>::Kick(())
                    });
                    break Ok(());
                }
                EventBufferState::Empty => {
                    break Ok(());
                }
            }
        }
    }

    #[ractor::message(DebouncerMsg::Enqueue(msg))]
    fn enqueue(
        &self,
        myself: ActorRef<DebouncerMsg<T>>,
        msg: T,
        state: &mut DebouncerState<T>,
    ) -> Result<()> {
        state.buffer.put(msg);
        ractor::cast!(myself, DebouncerMsg::<T>::Kick(())).anyerr()?;
        Ok(())
    }
}
