#pragma once

namespace qt_async {

// Creates the object that polls the Rust LocalSet. Call on the Qt main thread.
void
installPoller();

// Queues a poll of the Rust LocalSet on the Qt main thread. Thread-safe.
void
schedulePoll();

} // namespace qt_async
