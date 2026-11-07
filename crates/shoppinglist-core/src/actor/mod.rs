use n0_error::Result;
use ractor::{ActorRef, Message, RpcReplyPort};

pub mod debouncer;
mod stats;

pub use stats::*;

pub type Reply<T> = RpcReplyPort<Result<T>>;

#[track_caller]
pub fn tell<M, T>(
    actor: &ActorRef<M>,
    what: &'static str,
    build: impl FnOnce(Reply<T>) -> M + Send + 'static,
) where
    M: Message,
    T: Send + 'static,
{
    let actor = actor.clone();
    tokio::spawn(async move {
        match ractor::call!(actor, build) {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => tracing::warn!("{what} failed: {err:?}"),
            Err(err) => tracing::warn!("{what} failed: {err:?}"),
        }
    });
}

pub trait FlattenCall<T> {
    fn flatten_call(self) -> Result<T>;
}

impl<T, M> FlattenCall<T> for std::result::Result<Result<T>, ractor::RactorErr<M>> {
    fn flatten_call(self) -> Result<T> {
        match self {
            Ok(reply) => reply,
            Err(err) => Err(n0_error::anyerr!("request failed: {err:?}")),
        }
    }
}
