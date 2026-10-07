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

Deprecated ID-returning selectors and `DeploymentLease::into_deployment_id`
cancel shared admission through the synchronous bridge before returning the ID.
Failed writes retain the existing Drop cleanup and lease-expiry fallback. These
APIs retain the local active count until `release_deployment`, but an ID cannot
own a distributed reservation.
Keep the owned lease to enforce shared parallel, RPM, and TPM limits throughout
execution. Gateway execution retains owned leases.

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
immediately and places Redis cleanup in one bounded queue (1,024 entries, one
consumer on the admission runtime). Before reserving in Redis, each request owns
one of 1,024 process-wide cleanup slots until its hold or queued terminal operation
finishes. Saturation rejects new admission through the existing unavailable
result, so accepted holds always have space to retain known settlement usage.
The cleanup holds no HTTP-worker thread. Redis cleanup failures are still logged
and retain the existing lease-expiry fallback; this queue does not provide durable
token accounting during an outage.

Successful HTTP unary/streaming, SDK and runtime completion preserves known admission usage
and publishes shared circuit success before awaiting admission settlement.
Cancellation during that settlement cannot omit the already published outcome.

Gateway resources use the existing construction identity digest alongside the
deployment ID for shared Redis admission, circuit state and the circuit cache.
Replicas with the same resource share state; changing credentials or endpoints
isolates the replacement from retired leases and cooldowns. Old holds retain the
original namespace, so their completion cannot alter the replacement's quota.

Construction resolves native-audio environment keys/endpoints and Bedrock's current
static AWS credentials/session token/region before hashing and factory creation.
Explicit settings keep their provider precedence and error behavior. This uses
Bedrock's existing static/environment factory, not a dynamic IAM refresh service.

The synchronous owned-lease selectors preserve their synchronous drop behavior.

Regression tests exercise caller progress on a single-thread Tokio runtime for
both bridges, nested futures executors through the synchronous APIs, and a
real-Redis reservation whose caller is cancelled before its result is delivered.
They also cancel a known-usage settlement while all admission permits are busy
and check that drop cleanup records the actual tokens. Existing router,
streaming and Realtime regression suites cover normal finish, retries, counters
and generation admission. They are run by
the repository's existing CI. Provider/model budget operations use a separate
backend and are outside this async-routing change.

Vertex AI construction also captures the existing project/location environment fallbacks and the selected credential-file path before resource identity hashing. Explicit token or inline credentials retain precedence over an unused environment file. The identity also includes a digest of the parsed credentials held by the constructed provider, so same-path file content rotation changes identity without a second file read. This binds the configuration inputs used by the current factory; it does not claim identity discovery for a dynamically refreshed Application Default Credentials principal.

With shared Redis admission, runtime-backed SDK and DefaultRouter streams reserve the existing request token estimate plus the configured output bound. Once billable output has been observed, cancellation or EOF without trustworthy usage retains the RPM and estimated TPM reservation while releasing parallel admission. These counters remain conservative reservations, not reported actual token usage. Cancellation before output still refunds admission; observed provider usage settles the actual count. In-process admission records known RPM and observed tokens; it has no separate estimated-TPM reservation ledger.

Shared admission hashes expire after both the current quota minute and every live lease deadline. Ending one lease cannot remove another replica's live reservation or the current minute's settled/retained quota. Shared circuit calls refresh a ten-minute idle retention period, extended through any later open-circuit or probe-owner deadline; observing a circuit does not extend its open deadline. Active namespaces retain cumulative circuit counters. After the idle retention period, the shared circuit history starts fresh. This applies to newly created or subsequently accessed hashes; pre-upgrade idle keys without TTL are not backfilled.

Streaming provider failure preserves local failure and admission completion intent before notifying the shared circuit, then awaits admission settlement/cancellation. Cancellation during the later admission wait retains the captured failure and settlement responsibility. Circuit calls still use the existing bounded I/O bridge; a call cancelled before bridge admission has not been accepted for detached I/O.

When more stream output follows a usage snapshot, that snapshot no longer covers
the whole response. Shared admission retains the initial estimate until newer
usage covers the output, with any already observed larger count as a minimum.
A newer usage report, including an explicit zero, restores settlement to the
reported count. Local counters retain observed token counts. Anthropic adapters
expose complete, valid terminal usage; intermediate or incomplete token fields
remain unknown for settlement.

Runtime-backed SDK and DefaultRouter unary chat use the same input and output
estimate before provider dispatch. Runtime-backed SDK embeddings reserve the
existing input estimate for the whole single or batch request. Successful unary
responses without usage retain their shared token reservation; an explicit zero
or positive usage count settles to that reported count. Local token statistics
continue to include only observed usage.

Bedrock also contributes a digest of the static credentials held by the
constructed client. A fallback resolved during construction cannot share a
namespace with different credentials merely because earlier normalized inputs
matched. Vertex project normalization preserves the factory's existing
precedence for an explicit top-level project and provider-specific settings.

Accepted API-key usage writes outlive a cancelled request waiter. The key manager
owns at most 1,024 concurrent writes and retains the complete usage record,
including pricing and unpriced fields. Admission at that bound is best effort;
an overloaded writer returns an error, and process shutdown is not a durable
delivery guarantee. Normal callers continue to await the database result.
