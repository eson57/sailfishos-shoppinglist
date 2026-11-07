use std::pin::Pin;
use std::sync::Arc;

use n0_error::Result;

use super::Actors;

pub trait ActorWiringModel: Sized + 'static {
    /// Called on the Qt thread once the application's actors are available.
    fn connect_actors(self: Pin<&mut Self>, actors: Arc<Actors>);
}

pub fn wire_core_actors(_actors: &Actors) -> Result<()> {
    Ok(())
}
