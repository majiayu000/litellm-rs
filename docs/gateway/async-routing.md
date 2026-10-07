# Asynchronous shared routing state

Issue #1453 removes synchronous Redis waits from gateway deployment selection,
request completion and streaming lease cleanup. HTTP unary/streaming execution,
native Realtime generations, and runtime-owned execution now await admission and
circuit operations. Redis connections remain on their existing dedicated runtimes
so they outlive an individual Actix worker.

The public synchronous router selectors retain their signatures for existing Rust
consumers. They deliberately block until selection finishes. Async consumers can
use `select_deployment_lease_async` or
`select_deployment_lease_for_capability_matching_async`; a pinned `RuntimeHandle`
also exposes `select_deployment_lease_async`. Router-owned async execution uses
the same selector implementation, without the synchronous compatibility wrapper.
Synchronous query/record APIs also remain available for compatibility.
Their private waiter does not enter a futures executor, so they also work when
called from `futures::executor::block_on` or `LocalPool`. They still block the
calling thread; async callers should use the async entry points above.

Each admission/circuit bridge admits at most 64 I/O tasks. Additional callers wait
for an async semaphore permit rather than occupying an HTTP worker or creating
additional detached tasks. A launched operation retains its permit even if its
caller is cancelled. This change does not add a new request deadline or change
the configured Redis connection behavior.

The admission task constructs a shared RAII hold before returning its result.
If the request stops polling before receiving that result, the hold is still
dropped and releases its Redis reservation. If a request is cancelled while
waiting to settle known token usage, the hold retains settlement as its cleanup
operation instead of substituting cancellation.

Normal completion and retry cleanup are awaited before proceeding. A destructor
cannot await, so async lease drop releases the local active-request count
immediately and places Redis cleanup in an unbounded metadata queue with one
consumer on the admission runtime and no task per event. A slow Redis can grow
the process-local event backlog; it is not durable across process restarts.
The cleanup holds no HTTP-worker thread. If Redis fails or the consumer becomes
unavailable, the gateway logs the condition and retains the existing lease-expiry
safety fallback. Explicitly awaited completion
is preferred when the caller can still make progress. Queue fallback does not
claim durable token accounting during an outage.

The synchronous selectors preserve their synchronous drop behavior. Deprecated
ID-returning selectors retain their prior ownership contract; new code should
keep the lease rather than detach it into an ID.

Regression tests exercise caller progress on a single-thread Tokio runtime for
both bridges, nested futures executors through the synchronous APIs, and a
real-Redis reservation whose caller is cancelled before its result is delivered.
They also cancel a known-usage settlement while all admission permits are busy
and check that drop cleanup records the actual tokens. Existing router,
streaming and Realtime regression suites cover normal finish, retries, counters
and generation admission. They are run by
the repository's existing CI. Provider/model budget operations use a separate
backend and are outside this async-routing change.
