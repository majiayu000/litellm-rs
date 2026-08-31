import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { JSDOM } from "jsdom";
const indexHtml = await readFile(
  new URL("../../src/server/routes/admin_dashboard/index.html", import.meta.url),
  "utf8",
);
const appSource = await readFile(
  new URL("../../src/server/routes/admin_dashboard/app.js", import.meta.url),
  "utf8",
);
const providerHealthSource = await readFile(
  new URL(
    "../../src/server/routes/admin_dashboard/provider_health.js",
    import.meta.url,
  ),
  "utf8",
);
const immediate = () => new Promise((resolve) => setImmediate(resolve));
async function settle(turns = 8) {
  for (let index = 0; index < turns; index += 1) {
    await immediate();
  }
}
async function waitFor(predicate, message) {
  for (let index = 0; index < 80; index += 1) {
    if (predicate()) {
      return;
    }
    await immediate();
  }
  assert.fail(message);
}
function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}
function apiResponse(data, status = 200) {
  const success = status >= 200 && status < 300;
  return new Response(
    JSON.stringify(
      success ? { success: true, data } : { success: false, error: data },
    ),
    { status, headers: { "Content-Type": "application/json" } },
  );
}
function healthResponse(data, status = 200) {
  return new Response(JSON.stringify({ success: true, data }), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
function dashboard(options = {}) {
  const {
    keys = [],
    teams = [],
    usageByTeam = new Map(),
    health = {
      status: "degraded",
      reason: "no providers configured",
      timestamp: "2026-08-31T01:00:00Z",
      providers: {
        aggregate: "not_configured",
        healthy_providers: 0,
        total_providers: 0,
        enabled_providers: 0,
        provider_details: [],
      },
    },
    handler,
  } = options;
  const htmlWithoutScript = indexHtml
    .replace(
      '<script src="/admin/dashboard/provider-health.js" defer></script>',
      "",
    )
    .replace('<script src="/admin/dashboard/app.js" defer></script>', "");
  const dom = new JSDOM(htmlWithoutScript, {
    url: "https://gateway.test/admin/dashboard",
    runScripts: "outside-only",
  });
  const { window } = dom;
  const calls = [];
  const controllers = [];
  const clipboard = [];
  let confirm = () => true;
  class TrackingAbortController extends globalThis.AbortController {
    constructor() {
      super();
      this.abortCalls = 0;
      controllers.push(this);
    }
    abort(reason) {
      this.abortCalls += 1;
      super.abort(reason);
    }
  }
  Object.assign(window, {
    AbortController: TrackingAbortController,
    Headers,
    Request,
    Response,
  });
  Object.defineProperty(window.navigator, "clipboard", {
    configurable: true,
    value: {
      writeText: async (value) => {
        clipboard.push(value);
      },
    },
  });
  Object.defineProperty(window.HTMLDialogElement.prototype, "showModal", {
    configurable: true,
    value() {
      this.open = true;
    },
  });
  Object.defineProperty(window.HTMLDialogElement.prototype, "close", {
    configurable: true,
    value() {
      if (!this.open) {
        return;
      }
      this.open = false;
      this.dispatchEvent(new window.Event("close"));
    },
  });
  window.confirm = (...args) => confirm(...args);
  const context = {
    window,
    calls,
    controllers,
    clipboard,
    setConfirm(value) {
      confirm = typeof value === "function" ? value : () => value;
    },
  };
  window.fetch = (input, init = {}) => {
    const url = new URL(String(input), window.location.href);
    const call = {
      url,
      path: `${url.pathname}${url.search}`,
      method: String(init.method || "GET").toUpperCase(),
      init,
      controller: controllers.find((entry) => entry.signal === init.signal),
    };
    calls.push(call);
    const custom = handler?.(call, context);
    if (custom !== undefined) {
      return Promise.resolve(custom);
    }
    if (call.path === "/auth/login" && call.method === "POST") {
      return Promise.resolve(
        apiResponse({
          access_token: "session-token",
          user: { id: "admin-1", username: "operator", role: "admin" },
        }),
      );
    }
    if (call.path === "/auth/logout" && call.method === "POST") {
      return Promise.resolve(apiResponse(null));
    }
    if (url.pathname === "/v1/keys" && call.method === "GET") {
      return Promise.resolve(
        apiResponse({
          keys,
          pagination: { page: 1, pages: 1, has_prev: false, has_next: false },
        }),
      );
    }
    if (url.pathname === "/v1/teams" && call.method === "GET") {
      return Promise.resolve(
        apiResponse({
          items: teams,
          pagination: { page: 1, pages: 1, has_prev: false, has_next: false },
        }),
      );
    }
    if (url.pathname === "/health/detailed" && call.method === "GET") {
      return Promise.resolve(healthResponse(health, 503));
    }
    const usageMatch = url.pathname.match(/^\/v1\/teams\/([^/]+)\/usage$/);
    if (usageMatch && call.method === "GET") {
      const result = usageByTeam.get(decodeURIComponent(usageMatch[1]));
      return Promise.resolve(
        result instanceof Error
          ? apiResponse(result.message, 503)
          : apiResponse(result ?? {}),
      );
    }
    throw new Error(`Unexpected request: ${call.method} ${call.path}`);
  };
  window.eval(providerHealthSource);
  window.eval(appSource);
  return context;
}

function submit(window, form) {
  const button = form.querySelector('button[type="submit"]');
  const event = new window.Event("submit", {
    bubbles: true,
    cancelable: true,
  });
  Object.defineProperty(event, "submitter", { value: button });
  form.dispatchEvent(event);
  return button;
}

async function signIn(context) {
  const { window } = context;
  window.document.getElementById("username").value = "operator";
  window.document.getElementById("password").value = "password";
  submit(window, window.document.getElementById("login-form"));
  await waitFor(
    () =>
      !window.document.getElementById("dashboard-shell").hidden &&
      window.document.getElementById("status-region").textContent ===
        "Dashboard refreshed.",
    "dashboard did not finish signing in",
  );
}

function fillKeyForm(window, name) {
  window.document.getElementById("key-name").value = name;
  window.document.getElementById("key-models").value = "model-*";
  window.document.getElementById("key-endpoints").value =
    "/v1/chat/completions";
}

async function assertCopyCleared(context) {
  const before = [...context.clipboard];
  context.window.document.getElementById("copy-raw-key").click();
  await settle();
  assert.deepEqual(context.clipboard, before);
}

const rowText = (window, selector) =>
  [...window.document.querySelectorAll(selector)].map((row) => row.textContent);

test("B1 request/session generation ordering keeps only the newest refresh", { concurrency: false }, async (t) => {
  const pending = [];
  let keyGets = 0;
  let delayRefreshes = true;
  const context = dashboard({
    keys: [{ id: "fresh", name: "fresh-session", status: "inactive" }],
    handler(call) {
      if (call.url.pathname === "/v1/keys" && call.method === "GET") {
        keyGets += 1;
        if (keyGets > 1 && delayRefreshes) {
          const request = deferred();
          pending.push(request);
          return request.promise;
        }
      }
      return undefined;
    },
  });
  t.after(() => context.window.close());
  await signIn(context);

  const refresh = context.window.document.getElementById("refresh-keys");
  refresh.click();
  refresh.click();
  await waitFor(() => pending.length === 2, "two refreshes were not requested");
  pending[1].resolve(
    apiResponse({
      keys: [{ id: "new", name: "newest", status: "inactive" }],
      pagination: {},
    }),
  );
  await settle();
  pending[0].resolve(
    apiResponse({
      keys: [{ id: "old", name: "stale", status: "inactive" }],
      pagination: {},
    }),
  );
  await settle();

  const rows = context.window.document.querySelectorAll("#keys-body tr");
  assert.equal(rows.length, 1);
  assert.match(rows[0].textContent, /newest/);
  assert.doesNotMatch(rows[0].textContent, /stale/);

  refresh.click();
  await waitFor(() => pending.length === 3, "old-session refresh was not pending");
  context.window.document.getElementById("sign-out").click();
  delayRefreshes = false;
  await signIn(context);
  pending[2].resolve(
    apiResponse({
      keys: [{ id: "old-session", name: "old-session", status: "inactive" }],
      pagination: {},
    }),
  );
  await settle();
  const nextSessionRows =
    context.window.document.querySelectorAll("#keys-body tr");
  assert.equal(nextSessionRows.length, 1);
  assert.match(nextSessionRows[0].textContent, /fresh-session/);
  assert.doesNotMatch(nextSessionRows[0].textContent, /old-session/);
});

test("B2 controllers clean up on success/failure and logout aborts only active work", { concurrency: false }, async (t) => {
  const pendingKey = deferred();
  let keyGets = 0;
  let teamGets = 0;
  const context = dashboard({
    handler(call) {
      if (call.url.pathname === "/v1/keys" && call.method === "GET") {
        keyGets += 1;
        if (keyGets === 2) {
          return pendingKey.promise;
        }
      }
      if (call.url.pathname === "/v1/teams" && call.method === "GET") {
        teamGets += 1;
        if (teamGets === 2) {
          return Promise.reject(new Error("deterministic team failure"));
        }
      }
      return undefined;
    },
  });
  t.after(() => context.window.close());
  await signIn(context);

  context.window.document.getElementById("refresh-teams").click();
  await waitFor(
    () => teamGets === 2 && /failed/i.test(context.window.document.getElementById("status-region").textContent),
    "failed request did not settle",
  );
  const failedCall = context.calls.find(
    (call, index) =>
      call.url.pathname === "/v1/teams" &&
      context.calls
        .slice(0, index + 1)
        .filter((entry) => entry.url.pathname === "/v1/teams").length === 2,
  );
  context.window.document.getElementById("refresh-keys").click();
  await waitFor(() => keyGets === 2, "active refresh was not registered");
  const pendingCall = context.calls.find(
    (call, index) =>
      call.url.pathname === "/v1/keys" &&
      context.calls
        .slice(0, index + 1)
        .filter((entry) => entry.url.pathname === "/v1/keys").length === 2,
  );
  const completedControllers = context.calls
    .slice(0, 3)
    .map((call) => call.controller);

  context.window.document.getElementById("sign-out").click();
  await settle();
  assert.equal(failedCall.controller.abortCalls, 0);
  assert.equal(pendingCall.controller.abortCalls, 1);
  assert.ok(completedControllers.every((controller) => controller.abortCalls === 0));

  pendingKey.resolve(apiResponse({ keys: [], pagination: {} }));
  await settle();
  context.window.document.getElementById("sign-out").click();
  await settle();
  assert.equal(pendingCall.controller.abortCalls, 1);
});

test("B3 raw keys are one-time across copy, dismiss, close, logout, and late create", { concurrency: false }, async (t) => {
  const secrets = ["sk-copy-once", "sk-close-once", "sk-logout-once"];
  const lateCreate = deferred();
  let useLateCreate = false;
  const context = dashboard({
    handler(call) {
      if (call.url.pathname === "/v1/keys" && call.method === "POST") {
        return useLateCreate
          ? lateCreate.promise
          : apiResponse({ key: secrets.shift() });
      }
      return undefined;
    },
  });
  t.after(() => context.window.close());
  await signIn(context);
  const { window } = context;
  const dialog = window.document.getElementById("raw-key-dialog");

  fillKeyForm(window, "copy-key");
  submit(window, window.document.getElementById("create-key-form"));
  await waitFor(() => dialog.open, "raw key was not shown");
  window.document.getElementById("copy-raw-key").click();
  await settle();
  assert.deepEqual(context.clipboard, ["sk-copy-once"]);
  window.document.getElementById("dismiss-raw-key").click();
  assert.equal(dialog.open, false);
  assert.equal(window.document.getElementById("raw-key-value").textContent, "");
  await assertCopyCleared(context);

  fillKeyForm(window, "close-key");
  submit(window, window.document.getElementById("create-key-form"));
  await waitFor(() => dialog.open, "second raw key was not shown");
  dialog.close();
  assert.equal(window.document.getElementById("raw-key-value").textContent, "");
  await assertCopyCleared(context);

  fillKeyForm(window, "logout-key");
  submit(window, window.document.getElementById("create-key-form"));
  await waitFor(() => dialog.open, "third raw key was not shown");
  window.document.getElementById("sign-out").click();
  assert.equal(dialog.open, false);
  assert.equal(window.document.getElementById("raw-key-value").textContent, "");
  await assertCopyCleared(context);

  await signIn(context);
  useLateCreate = true;
  const createsBeforeLateRequest = context.calls.filter(
    (call) => call.url.pathname === "/v1/keys" && call.method === "POST",
  ).length;
  fillKeyForm(window, "late-key");
  submit(window, window.document.getElementById("create-key-form"));
  await waitFor(
    () =>
      context.calls.filter(
        (call) => call.url.pathname === "/v1/keys" && call.method === "POST",
      ).length ===
      createsBeforeLateRequest + 1,
    "late create was not requested",
  );
  window.document.getElementById("sign-out").click();
  lateCreate.resolve(apiResponse({ key: "sk-never-visible" }));
  await settle();
  assert.equal(dialog.open, false);
  assert.equal(window.document.body.textContent.includes("sk-never-visible"), false);
});

test("B4 mixed team usage keeps successful explicit zero and marks only failure", { concurrency: false }, async (t) => {
  const context = dashboard({
    teams: [
      { id: "team-zero", name: "zero-team", status: "active", member_count: 1 },
      { id: "team-fail", name: "failed-team", status: "active", member_count: 1 },
    ],
    usageByTeam: new Map([
      [
        "team-zero",
        { cost_today: 0, total_cost: 0, total_requests: 0, total_tokens: 0 },
      ],
      ["team-fail", new Error("usage unavailable")],
    ]),
  });
  t.after(() => context.window.close());
  await signIn(context);

  const rows = [...context.window.document.querySelectorAll("#team-spend-body tr")];
  assert.equal(rows.length, 2);
  assert.deepEqual(
    [...rows[0].querySelectorAll("td")].map((cell) => cell.textContent),
    ["zero-team", "$0.0000", "$0.0000", "0", "0"],
  );
  assert.match(rows[1].textContent, /failed-team/);
  assert.match(rows[1].textContent, /usage unavailable/);
  assert.equal(rows[1].querySelectorAll("td").length, 2);
});

test("B5 destructive actions require confirmation and disable exactly one pending DELETE", { concurrency: false }, async (t) => {
  const pendingDeletes = [];
  const context = dashboard({
    keys: [{ id: "key-1", name: "key-one", status: "active" }],
    teams: [{ id: "team-1", name: "team-one", status: "active", member_count: 1 }],
    handler(call) {
      if (call.method === "DELETE") {
        const request = deferred();
        pendingDeletes.push({ call, request });
        return request.promise;
      }
      return undefined;
    },
  });
  t.after(() => context.window.close());
  await signIn(context);
  const { window } = context;
  context.setConfirm(false);
  const rowsBeforeCancel = [
    rowText(window, "#keys-body tr"),
    rowText(window, "#teams-body tr"),
  ];
  window.document.querySelector("#keys-body button").click();
  window.document.querySelector("#teams-body button").click();
  await settle();
  assert.equal(pendingDeletes.length, 0);
  assert.deepEqual(
    [rowText(window, "#keys-body tr"), rowText(window, "#teams-body tr")],
    rowsBeforeCancel,
  );

  context.setConfirm(true);
  const revoke = window.document.querySelector("#keys-body button");
  revoke.click();
  await waitFor(() => pendingDeletes.length === 1, "revoke was not requested");
  assert.equal(revoke.disabled, true);
  revoke.click();
  assert.equal(
    context.calls.filter(
      (call) => call.method === "DELETE" && call.url.pathname === "/v1/keys/key-1",
    ).length,
    1,
  );
  pendingDeletes[0].request.resolve(apiResponse(null));
  await waitFor(() => !revoke.disabled, "revoke control was not restored");

  const remove = window.document.querySelector("#teams-body button");
  remove.click();
  await waitFor(() => pendingDeletes.length === 2, "delete was not requested");
  assert.equal(remove.disabled, true);
  remove.click();
  assert.equal(
    context.calls.filter(
      (call) => call.method === "DELETE" && call.url.pathname === "/v1/teams/team-1",
    ).length,
    1,
  );
  pendingDeletes[1].request.resolve(apiResponse(null));
  await waitFor(() => !remove.disabled, "delete control was not restored");
});

test("B6 late refresh/create/delete responses after logout restore no protected state", { concurrency: false }, async (t) => {
  const lateLogin = deferred();
  const lateRefresh = deferred();
  const lateCreate = deferred();
  const lateDelete = deferred();
  const lateUsage = deferred();
  let loginPosts = 0;
  let keyGets = 0;
  let usageGets = 0;
  const context = dashboard({
    keys: [{ id: "key-late", name: "visible-key", status: "active" }],
    teams: [{ id: "team-late", name: "visible-team", status: "active", member_count: 1 }],
    handler(call) {
      if (call.url.pathname === "/auth/login" && call.method === "POST") {
        loginPosts += 1;
        if (loginPosts === 2) {
          return lateLogin.promise;
        }
      }
      if (call.url.pathname === "/v1/keys" && call.method === "GET") {
        keyGets += 1;
        if (keyGets === 2) {
          return lateRefresh.promise;
        }
      }
      if (call.url.pathname === "/v1/keys" && call.method === "POST") {
        return lateCreate.promise;
      }
      if (
        call.url.pathname === "/v1/teams/team-late/usage" &&
        call.method === "GET"
      ) {
        usageGets += 1;
        if (usageGets === 2) {
          return lateUsage.promise;
        }
      }
      if (
        call.url.pathname === "/v1/teams/team-late" &&
        call.method === "DELETE"
      ) {
        return lateDelete.promise;
      }
      return undefined;
    },
  });
  t.after(() => context.window.close());
  await signIn(context);
  const { window } = context;
  context.setConfirm(true);
  window.document.getElementById("refresh-teams").click();
  await waitFor(() => usageGets === 2, "late usage request was not pending");
  window.document.getElementById("refresh-keys").click();
  fillKeyForm(window, "late-created-key");
  submit(window, window.document.getElementById("create-key-form"));
  window.document.querySelector("#teams-body button").click();
  window.document.getElementById("password").value = "late-password";
  submit(window, window.document.getElementById("login-form"));
  await waitFor(() => loginPosts === 2 && keyGets === 2, "late operations were not all pending");

  window.document.getElementById("sign-out").click();
  const callsAtResolution = context.calls.length;
  lateLogin.resolve(
    apiResponse({
      access_token: "late-session-token",
      user: { id: "late-admin", username: "late-operator", role: "admin" },
    }),
  );
  lateRefresh.resolve(
    apiResponse({
      keys: [{ id: "restored", name: "must-not-return", status: "active" }],
      pagination: {},
    }),
  );
  lateCreate.resolve(apiResponse({ key: "sk-late-secret" }));
  lateDelete.resolve(apiResponse(null));
  lateUsage.resolve(
    apiResponse({
      cost_today: 999,
      total_cost: 999,
      total_requests: 999,
      total_tokens: 999,
    }),
  );
  await settle(12);

  assert.equal(context.calls.length, callsAtResolution);
  assert.equal(window.document.getElementById("password").value, "");
  assert.equal(window.document.getElementById("login-panel").hidden, false);
  assert.equal(window.document.getElementById("dashboard-shell").hidden, true);
  assert.equal(window.document.getElementById("sign-out").hidden, true);
  assert.equal(window.document.getElementById("session-label").textContent, "Signed out");
  assert.equal(window.document.querySelectorAll("#keys-body tr").length, 0);
  assert.equal(window.document.querySelectorAll("#teams-body tr").length, 0);
  assert.equal(window.document.getElementById("raw-key-dialog").open, false);
  assert.equal(window.document.getElementById("raw-key-value").textContent, "");
  assert.doesNotMatch(
    window.document.body.textContent,
    /must-not-return|sk-late-secret|late-operator|\$999/,
  );
  assert.match(
    window.document.getElementById("status-region").textContent,
    /^Signed out\./,
  );
});

test("B7 provider health refresh renders enabled and probe states from a 503 snapshot", { concurrency: false }, async (t) => {
  const refreshedHealth = deferred();
  let healthGets = 0;
  const context = dashboard({
    handler(call) {
      if (call.url.pathname === "/health/detailed" && call.method === "GET") {
        healthGets += 1;
        if (healthGets === 2) {
          return refreshedHealth.promise;
        }
      }
      return undefined;
    },
  });
  t.after(() => context.window.close());
  await signIn(context);
  const { window } = context;

  const tab = window.document.querySelector('[data-view="health"]');
  assert.ok(tab, "provider health tab is missing");
  tab.click();
  const refresh = window.document.getElementById("refresh-health");
  assert.equal(refresh.type, "button");
  assert.match(refresh.textContent, /refresh/i);
  refresh.click();
  await waitFor(() => healthGets === 2, "provider health refresh was not requested");
  assert.equal(refresh.disabled, true);

  refreshedHealth.resolve(
    healthResponse(
      {
        status: "degraded",
        reason: "one or more providers unhealthy",
        timestamp: "2026-08-31T02:03:04Z",
        providers: {
          aggregate: "degraded",
          healthy_providers: 1,
          total_providers: 4,
          enabled_providers: 3,
          provider_details: [
            {
              name: "openai",
              status: "healthy",
              last_check: "2026-08-31T02:03:00Z",
              response_time_ms: 18,
              error_message: null,
            },
            {
              name: "anthropic",
              status: "unknown",
              last_check: null,
              response_time_ms: null,
              error_message: "upstream health has not been established yet",
            },
            {
              name: "gemini",
              status: "unhealthy",
              last_check: "2026-08-31T02:02:30Z",
              response_time_ms: null,
              error_message: "probe failed",
            },
            {
              name: "ollama",
              status: "disabled",
              last_check: null,
              response_time_ms: null,
              error_message: null,
            },
          ],
        },
      },
      503,
    ),
  );
  await waitFor(() => !refresh.disabled, "provider health refresh did not settle");

  assert.match(window.document.getElementById("health-summary").textContent, /3 enabled/i);
  assert.match(window.document.getElementById("health-summary").textContent, /1 healthy/i);
  assert.match(window.document.getElementById("health-notice").textContent, /HTTP 503/i);
  assert.match(window.document.getElementById("health-notice").textContent, /unknown/i);
  const rows = rowText(window, "#health-body tr");
  assert.equal(rows.length, 4);
  assert.match(rows[0], /openai.*enabled.*healthy/i);
  assert.match(rows[0], /8\/31\/2026|31\/8\/2026/);
  assert.match(rows[1], /anthropic.*enabled.*unknown.*not probed/i);
  assert.match(rows[2], /gemini.*enabled.*unhealthy.*probe failed/i);
  assert.match(rows[3], /ollama.*disabled/i);
});

test("B8 provider health failures are explicit and clear stale health", { concurrency: false }, async (t) => {
  let mode = "healthy";
  const context = dashboard({
    health: {
      status: "healthy",
      reason: "ok",
      timestamp: "2026-08-31T01:00:00Z",
      providers: {
        aggregate: "healthy",
        healthy_providers: 1,
        total_providers: 1,
        enabled_providers: 1,
        provider_details: [
          { name: "openai", status: "healthy", last_check: null },
        ],
      },
    },
    handler(call) {
      if (call.url.pathname !== "/health/detailed" || call.method !== "GET") {
        return undefined;
      }
      if (mode === "network") {
        return Promise.reject(new Error("network offline"));
      }
      if (mode === "unauthorized") {
        return apiResponse("expired", 401);
      }
      return undefined;
    },
  });
  t.after(() => context.window.close());
  await signIn(context);
  const { window } = context;
  window.document.querySelector('[data-view="health"]').click();
  assert.equal(window.document.querySelectorAll("#health-body tr").length, 1);

  mode = "network";
  window.document.getElementById("refresh-health").click();
  await waitFor(
    () => /network offline/i.test(window.document.getElementById("health-notice").textContent),
    "network failure was not shown in the health view",
  );
  assert.equal(window.document.querySelectorAll("#health-body tr").length, 0);
  assert.match(window.document.getElementById("error-region").textContent, /network offline/i);

  mode = "unauthorized";
  window.document.getElementById("refresh-health").click();
  await waitFor(
    () => !window.document.getElementById("login-panel").hidden,
    "401 did not expire the administrator session",
  );
  assert.equal(window.document.getElementById("dashboard-shell").hidden, true);
  assert.equal(window.document.getElementById("health-summary").textContent, "");
  assert.equal(window.document.getElementById("health-notice").textContent, "");
  assert.equal(window.document.querySelectorAll("#health-body tr").length, 0);
  assert.match(window.document.getElementById("error-region").textContent, /session expired/i);
});

test("B9 a late provider health response after logout cannot restore stale data", { concurrency: false }, async (t) => {
  const lateHealth = deferred();
  let healthGets = 0;
  const context = dashboard({
    handler(call) {
      if (call.url.pathname === "/health/detailed" && call.method === "GET") {
        healthGets += 1;
        if (healthGets === 2) {
          return lateHealth.promise;
        }
      }
      return undefined;
    },
  });
  t.after(() => context.window.close());
  await signIn(context);
  const { window } = context;
  window.document.querySelector('[data-view="health"]').click();
  window.document.getElementById("refresh-health").click();
  await waitFor(() => healthGets === 2, "late provider health request was not pending");

  window.document.getElementById("sign-out").click();
  lateHealth.resolve(
    healthResponse({
      status: "healthy",
      reason: "ok",
      timestamp: "2026-08-31T03:00:00Z",
      providers: {
        aggregate: "healthy",
        healthy_providers: 1,
        total_providers: 1,
        enabled_providers: 1,
        provider_details: [
          { name: "must-not-return", status: "healthy", last_check: null },
        ],
      },
    }),
  );
  await settle(12);

  assert.equal(window.document.getElementById("dashboard-shell").hidden, true);
  assert.equal(window.document.getElementById("health-summary").textContent, "");
  assert.equal(window.document.getElementById("health-notice").textContent, "");
  assert.equal(window.document.querySelectorAll("#health-body tr").length, 0);
  assert.doesNotMatch(window.document.body.textContent, /must-not-return/);
});
