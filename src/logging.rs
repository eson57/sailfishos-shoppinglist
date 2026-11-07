use std::{
    collections::VecDeque,
    io,
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
};

use tokio::sync::mpsc;
use tracing::{Level, Metadata, Subscriber};
use tracing_subscriber::{
    Layer,
    filter::filter_fn,
    fmt::{self, MakeWriter, time::ChronoLocal},
    registry::LookupSpan,
};

pub const CAPACITY: usize = 500;

#[derive(Debug, Clone)]
pub struct LogLine {
    pub level: Level,
    pub text: String,
}

#[derive(Default)]
struct Inner {
    recent: VecDeque<LogLine>,
    subscribers: Vec<mpsc::UnboundedSender<LogLine>>,
}

#[derive(Default)]
pub struct LogBuffer {
    inner: Mutex<Inner>,
}

pub struct LogSubscription {
    pub history: Vec<LogLine>,
    pub new_lines: mpsc::UnboundedReceiver<LogLine>,
}

static BUFFER: OnceLock<LogBuffer> = OnceLock::new();

pub fn buffer() -> &'static LogBuffer {
    BUFFER.get_or_init(LogBuffer::default)
}

impl LogBuffer {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push(&self, line: LogLine) {
        let mut inner = self.inner();
        if inner.recent.len() == CAPACITY {
            inner.recent.pop_front();
        }
        inner.recent.push_back(line.clone());
        inner
            .subscribers
            .retain(|subscriber| subscriber.send(line.clone()).is_ok());
    }

    pub fn subscribe(&self) -> LogSubscription {
        let (sender, new_lines) = mpsc::unbounded_channel();

        let mut inner = self.inner();
        let history = inner.recent.iter().cloned().collect();
        inner
            .subscribers
            .retain(|subscriber| !subscriber.is_closed());
        inner.subscribers.push(sender);

        LogSubscription { history, new_lines }
    }
}

/// Events with this target are not collected by the in-app logger.
///
/// This should be used for all log events emitted by the logging page as to not trigger a loop.
///
/// ```ignore
/// tracing::debug!(target: crate::logging::UNCOLLECTED, "applied {n} log lines");
/// ```
pub const UNCOLLECTED: &str = "uncollected";

/// A formatting layer that writes into [`buffer`].
pub fn layer<S>() -> impl Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fmt::layer()
        .with_ansi(false)
        .with_timer(ChronoLocal::new("%H:%M:%S%.3f".to_string()))
        .with_writer(BufferWriter)
        .with_filter(filter_fn(|metadata| metadata.target() != UNCOLLECTED))
}

struct BufferWriter;

impl<'a> MakeWriter<'a> for BufferWriter {
    type Writer = LineWriter;

    fn make_writer(&'a self) -> LineWriter {
        LineWriter::new(Level::INFO)
    }

    fn make_writer_for(&'a self, meta: &Metadata<'_>) -> LineWriter {
        LineWriter::new(*meta.level())
    }
}

struct LineWriter {
    level: Level,
    bytes: Vec<u8>,
}

impl LineWriter {
    fn new(level: Level) -> Self {
        Self {
            level,
            bytes: Vec::new(),
        }
    }
}

impl io::Write for LineWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for LineWriter {
    fn drop(&mut self) {
        let text = String::from_utf8_lossy(&self.bytes);
        let text = text.trim_end();
        if !text.is_empty() {
            buffer().push(LogLine {
                level: self.level,
                text: text.to_owned(),
            });
        }
    }
}
