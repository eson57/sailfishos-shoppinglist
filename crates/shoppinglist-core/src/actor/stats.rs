use std::time::{Duration, Instant, SystemTime};

use ractor::{
    Actor, ActorCell, ActorId, ActorProcessingErr, ActorRef, ActorStatus, SupervisionEvent,
};

#[derive(Debug, Clone)]
pub struct ActorStats {
    name: &'static str,
    started_at: Instant,
    started_wall: SystemTime,
    last_message: Option<Instant>,
    messages: u64,
}

impl ActorStats {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            started_at: Instant::now(),
            started_wall: SystemTime::now(),
            last_message: None,
            messages: 0,
        }
    }

    fn touch(&mut self) {
        self.last_message = Some(Instant::now());
        self.messages += 1;
    }

    pub fn snapshot(&self, cell: &ActorCell) -> ActorStatsSnapshot {
        ActorStatsSnapshot {
            name: self.name,
            id: cell.get_id(),
            status: cell.get_status(),
            started_at: self.started_wall,
            uptime: self.started_at.elapsed(),
            since_last_message: self.last_message.map(|at| at.elapsed()),
            messages: self.messages,
        }
    }
}

impl Default for ActorStats {
    fn default() -> Self {
        Self::new("unnamed")
    }
}

/// Actor stats at a point in time
#[derive(Debug, Clone)]
pub struct ActorStatsSnapshot {
    pub name: &'static str,
    pub id: ActorId,
    pub status: ActorStatus,
    pub started_at: SystemTime,
    pub uptime: Duration,
    pub since_last_message: Option<Duration>,
    pub messages: u64,
}

pub trait HasStats {
    fn stats_mut(&mut self) -> &mut ActorStats;
}

/// Trait used by [Instrumented] to decide if a message should be counted for stats.
/// Use this to ignore the message you use for retrieving stats.
pub trait ShouldMeasureMessage {
    fn should_measure(&self) -> bool;
}

/// Actor wrapper which counts how many messages it receives
pub struct Instrumented<A>(pub A);

impl<A> Actor for Instrumented<A>
where
    A: Actor,
    A::Msg: ShouldMeasureMessage,
    A::State: HasStats,
{
    type Msg = A::Msg;
    type State = A::State;
    type Arguments = A::Arguments;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        self.0.pre_start(myself, args).await
    }

    async fn post_start(
        &self,
        myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        self.0.post_start(myself, state).await
    }

    async fn post_stop(
        &self,
        myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        self.0.post_stop(myself, state).await
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        if message.should_measure() {
            state.stats_mut().touch();
        }
        self.0.handle(myself, message, state).await
    }

    async fn handle_supervisor_evt(
        &self,
        myself: ActorRef<Self::Msg>,
        message: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        state.stats_mut().touch();
        self.0.handle_supervisor_evt(myself, message, state).await
    }
}
