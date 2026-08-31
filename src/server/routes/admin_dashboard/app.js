"use strict";
const PAGE_SIZE = 20;
const state = {
  token: null,
  adminId: null,
  adminName: null,
  generation: 0,
  controllers: new Set(),
  busy: new Set(),
  currentView: "keys",
  keyPage: 1,
  teamPage: 1,
  keys: [],
  teams: [],
  keyPagination: {},
  teamPagination: {},
  teamUsage: new Map(),
  rawKey: null,
  keyRequestVersion: 0,
  teamRequestVersion: 0,
  usageRequestVersion: 0,
};
const byId = (id) => document.getElementById(id);
const loginPanel = byId("login-panel");
const dashboardShell = byId("dashboard-shell");
const errorRegion = byId("error-region");
const statusRegion = byId("status-region");
const rawKeyDialog = byId("raw-key-dialog");
function setStatus(message) {
  statusRegion.textContent = message || "";
}
function setError(error) {
  const message = error instanceof Error ? error.message : String(error || "");
  errorRegion.textContent = message;
  errorRegion.hidden = !message;
}
function clearError() {
  setError("");
}
function reportRequestError(error, message) {
  if (error?.name === "AbortError") {
    return;
  }
  setError(error);
  setStatus(message);
}
function abortActiveRequests() {
  for (const controller of state.controllers) {
    controller.abort();
  }
  state.controllers.clear();
}
function captureSession() {
  if (!state.token || !state.adminId) {
    throw new Error("Administrator authentication is required.");
  }
  return { token: state.token, generation: state.generation };
}
function ensureCurrent(session) {
  if (
    session.token !== state.token ||
    session.generation !== state.generation
  ) {
    throw new DOMException("Stale dashboard response", "AbortError");
  }
}
function resetProtectedState() {
  state.keys = [];
  state.teams = [];
  state.keyPagination = {};
  state.teamPagination = {};
  state.teamUsage = new Map();
  clearRawKey();
  renderKeys();
  renderTeams();
  renderSpend();
  providerHealthView.reset();
}
function endSession(message = "Signed out") {
  abortActiveRequests();
  state.generation += 1;
  state.token = null;
  state.adminId = null;
  state.adminName = null;
  state.busy.clear();
  resetProtectedState();
  loginPanel.hidden = false;
  dashboardShell.hidden = true;
  byId("sign-out").hidden = true;
  byId("session-label").textContent = message;
  byId("password").value = "";
}
async function decodeResponse(response, acceptedStatuses = []) {
  const text = await response.text();
  let payload = null;
  if (text) {
    try {
      payload = JSON.parse(text);
    } catch {
      if (!response.ok && !acceptedStatuses.includes(response.status)) {
        throw new Error(text.trim() || `Gateway request failed (${response.status}).`);
      }
      throw new Error(`Gateway returned invalid JSON (${response.status}).`);
    }
  }
  if (
    (!response.ok && !acceptedStatuses.includes(response.status)) ||
    payload?.success === false
  ) {
    const structuredError =
      typeof payload?.error === "string"
        ? payload.error
        : typeof payload?.error?.message === "string"
          ? payload.error.message
          : typeof payload?.message === "string"
            ? payload.message
            : null;
    throw new Error(
      structuredError || `Gateway request failed (${response.status}).`,
    );
  }
  return payload?.data ?? payload;
}
async function publicRequest(path, options, generation) {
  const controller = new AbortController();
  state.controllers.add(controller);
  try {
    const response = await fetch(path, { ...options, signal: controller.signal });
    if (generation !== state.generation) {
      throw new DOMException("Stale login response", "AbortError");
    }
    return await decodeResponse(response);
  } finally {
    state.controllers.delete(controller);
  }
}
async function apiRequest(
  path,
  options = {},
  session = captureSession(),
  responsePolicy = {},
) {
  const controller = new AbortController();
  state.controllers.add(controller);
  const headers = new Headers(options.headers || {});
  headers.set("Accept", "application/json");
  headers.set("Authorization", `Bearer ${session.token}`);
  if (options.body) {
    headers.set("Content-Type", "application/json");
  }
  try {
    const response = await fetch(path, {
      ...options,
      headers,
      signal: controller.signal,
    });
    if (response.status === 401) {
      ensureCurrent(session);
      endSession("Session expired");
      setError("Your administrator session expired. Sign in again.");
      setStatus("Session expired. Protected dashboard data was cleared.");
      throw new DOMException("Administrator session expired", "AbortError");
    }
    const data = await decodeResponse(
      response,
      responsePolicy.acceptedStatuses || [],
    );
    ensureCurrent(session);
    return responsePolicy.includeStatus
      ? { data, status: response.status }
      : data;
  } finally {
    state.controllers.delete(controller);
  }
}
async function logoutRequest(token, generation) {
  const controller = new AbortController();
  state.controllers.add(controller);
  try {
    const response = await fetch("/auth/logout", {
      method: "POST",
      headers: { Authorization: `Bearer ${token}`, Accept: "application/json" },
      signal: controller.signal,
    });
    if (generation !== state.generation) {
      throw new DOMException("Stale logout response", "AbortError");
    }
    if (!response.ok) {
      throw new Error(`Server logout failed (${response.status}).`);
    }
  } finally {
    state.controllers.delete(controller);
  }
}
function beginBusy(key, button) {
  if (state.busy.has(key)) {
    return false;
  }
  state.busy.add(key);
  if (button) {
    button.disabled = true;
  }
  return true;
}
function endBusy(key, button) {
  state.busy.delete(key);
  if (button) {
    button.disabled = false;
  }
}
function splitScope(value, label) {
  const values = value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
  if (!values.length) {
    throw new Error(`${label} requires at least one value.`);
  }
  if (values.some((value) => value === "*")) {
    throw new Error(`${label} does not accept the unrestricted * value.`);
  }
  return values;
}
function textCell(value) {
  const cell = document.createElement("td");
  cell.textContent = value == null ? "" : String(value);
  return cell;
}
function actionCell(button) {
  const cell = document.createElement("td");
  if (button) {
    cell.append(button);
  }
  return cell;
}
function formatDate(value) {
  if (!value) {
    return "";
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleDateString();
}
function formatNumber(value) {
  if (value == null || value === "") {
    return "";
  }
  const number = typeof value === "number" ? value : Number(value);
  return Number.isFinite(number) ? number.toLocaleString() : "";
}
function formatMoney(value) {
  if (value == null || value === "") {
    return "";
  }
  const number = typeof value === "number" ? value : Number(value);
  return Number.isFinite(number) ? `$${number.toFixed(4)}` : "";
}
function keyOwner(key) {
  if (key.team_id) {
    return `Team ${key.team_id}`;
  }
  if (key.user_id) {
    return `User ${key.user_id}`;
  }
  return "";
}
function createAction(label, className, handler) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.className = className;
  button.addEventListener("click", handler);
  return button;
}
function renderKeys() {
  const body = byId("keys-body");
  body.replaceChildren();
  for (const key of state.keys) {
    const row = document.createElement("tr");
    row.append(
      textCell(key.name),
      textCell(key.key_prefix),
      textCell(keyOwner(key)),
      textCell(key.status),
      textCell(formatDate(key.created_at)),
    );
    let action = null;
    if (String(key.status).toLowerCase() === "active") {
      action = createAction("Revoke", "danger", (event) => {
        void revokeKey(key, event.currentTarget);
      });
    }
    row.append(actionCell(action));
    body.append(row);
  }
  byId("keys-empty").hidden = state.keys.length !== 0;
  const page = state.keyPagination.page || state.keyPage;
  const pages = state.keyPagination.pages || 1;
  byId("keys-page").textContent = `Page ${page} of ${Math.max(pages, 1)}`;
  byId("keys-previous").disabled = !state.keyPagination.has_prev;
  byId("keys-next").disabled = !state.keyPagination.has_next;
}
function renderTeams() {
  const body = byId("teams-body");
  body.replaceChildren();
  for (const team of state.teams) {
    const row = document.createElement("tr");
    const remove = createAction("Delete", "danger", (event) => {
      void deleteTeam(team, event.currentTarget);
    });
    row.append(
      textCell(team.name),
      textCell(team.display_name),
      textCell(team.status),
      textCell(team.member_count),
      actionCell(remove),
    );
    body.append(row);
  }
  byId("teams-empty").hidden = state.teams.length !== 0;
  const page = state.teamPagination.page || state.teamPage;
  const pages = state.teamPagination.pages || 1;
  byId("teams-page").textContent = `Page ${page} of ${Math.max(pages, 1)}`;
  byId("teams-previous").disabled = !state.teamPagination.has_prev;
  byId("teams-next").disabled = !state.teamPagination.has_next;
  renderTeamOptions();
}
function renderTeamOptions() {
  const select = byId("key-team");
  const selected = select.value;
  const personal = document.createElement("option");
  personal.value = "";
  personal.textContent = "My administrator user";
  const options = [personal];
  for (const team of state.teams) {
    const option = document.createElement("option");
    option.value = team.id;
    option.textContent = team.display_name || team.name;
    options.push(option);
  }
  select.replaceChildren(...options);
  if (options.some((option) => option.value === selected)) {
    select.value = selected;
  }
}
function usageRow(label, usage, error) {
  const row = document.createElement("tr");
  row.append(textCell(label));
  if (error || !usage) {
    const cell = textCell("");
    cell.colSpan = 4;
    cell.textContent = error || "Usage unavailable";
    row.append(cell);
    return row;
  }
  row.append(
    textCell(formatMoney(usage.cost_today)),
    textCell(formatMoney(usage.total_cost)),
    textCell(formatNumber(usage.total_requests)),
    textCell(formatNumber(usage.total_tokens)),
  );
  return row;
}
function renderSpend() {
  const keyBody = byId("key-spend-body");
  keyBody.replaceChildren(
    ...state.keys.map((key) => usageRow(key.name, key.usage_stats, null)),
  );
  byId("key-spend-empty").hidden = state.keys.length !== 0;
  const teamBody = byId("team-spend-body");
  teamBody.replaceChildren(
    ...state.teams.map((team) => {
      const entry = state.teamUsage.get(team.id);
      return usageRow(team.display_name || team.name, entry?.usage, entry?.error);
    }),
  );
  byId("team-spend-empty").hidden = state.teams.length !== 0;
}
const providerHealthView = window.createProviderHealthView({
  apiRequest,
  byId,
  captureSession,
  clearError,
  ensureCurrent,
  reportRequestError,
  setStatus,
  textCell,
});
async function loadKeys(session = captureSession()) {
  const requestVersion = ++state.keyRequestVersion;
  const requestedPage = state.keyPage;
  const data = await apiRequest(
    `/v1/keys?page=${requestedPage}&limit=${PAGE_SIZE}`,
    {},
    session,
  );
  ensureCurrent(session);
  if (
    requestVersion !== state.keyRequestVersion ||
    requestedPage !== state.keyPage
  ) {
    throw new DOMException("Stale API key response", "AbortError");
  }
  state.keys = Array.isArray(data?.keys) ? data.keys : [];
  state.keyPagination = data?.pagination || {};
  renderKeys();
  renderSpend();
}
async function loadTeams(session = captureSession()) {
  const requestVersion = ++state.teamRequestVersion;
  const requestedPage = state.teamPage;
  state.usageRequestVersion += 1;
  const data = await apiRequest(
    `/v1/teams?page=${requestedPage}&limit=${PAGE_SIZE}`,
    {},
    session,
  );
  ensureCurrent(session);
  if (
    requestVersion !== state.teamRequestVersion ||
    requestedPage !== state.teamPage
  ) {
    throw new DOMException("Stale team response", "AbortError");
  }
  state.teams = Array.isArray(data?.items) ? data.items : [];
  state.teamPagination = data?.pagination || {};
  state.teamUsage = new Map();
  renderTeams();
  renderSpend();
}
async function loadTeamUsage(session = captureSession()) {
  const requestVersion = ++state.usageRequestVersion;
  const teams = [...state.teams];
  const results = await Promise.allSettled(
    teams.map((team) =>
      apiRequest(`/v1/teams/${encodeURIComponent(team.id)}/usage`, {}, session),
    ),
  );
  ensureCurrent(session);
  if (
    requestVersion !== state.usageRequestVersion ||
    teams.length !== state.teams.length ||
    teams.some((team, index) => team.id !== state.teams[index]?.id)
  ) {
    throw new DOMException("Stale team usage response", "AbortError");
  }
  const usage = new Map();
  results.forEach((result, index) => {
    const team = teams[index];
    if (result.status === "fulfilled") {
      usage.set(team.id, { usage: result.value, error: null });
    } else {
      usage.set(team.id, {
        usage: null,
        error:
          result.reason?.name === "AbortError"
            ? "Usage request cancelled"
            : result.reason?.message || "Usage unavailable",
      });
    }
  });
  state.teamUsage = usage;
  renderSpend();
}
async function refreshDashboard() {
  const session = captureSession();
  clearError();
  setStatus("Refreshing dashboard…");
  try {
    await Promise.all([
      loadKeys(session),
      loadTeams(session),
      providerHealthView.load(session),
    ]);
    ensureCurrent(session);
    await loadTeamUsage(session);
    ensureCurrent(session);
    setStatus("Dashboard refreshed.");
  } catch (error) {
    if (error?.name !== "AbortError") {
      setError(error);
      setStatus("Dashboard refresh incomplete.");
    }
  }
}
async function signIn(event) {
  event.preventDefault();
  const button = event.submitter;
  if (!beginBusy("login", button)) {
    return;
  }
  abortActiveRequests();
  state.generation += 1;
  const generation = state.generation;
  clearError();
  setStatus("Signing in…");
  try {
    const data = await publicRequest(
      "/auth/login",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify({
          username: byId("username").value,
          password: byId("password").value,
        }),
      },
      generation,
    );
    if (String(data?.user?.role).toLowerCase() !== "admin") {
      throw new Error("Administrator privileges are required.");
    }
    if (!data?.access_token || !data?.user?.id) {
      throw new Error("Login response did not include an administrator session.");
    }
    if (generation !== state.generation) {
      throw new DOMException("Stale login response", "AbortError");
    }
    state.token = data.access_token;
    state.adminId = data.user.id;
    state.adminName = data.user.username;
    byId("password").value = "";
    loginPanel.hidden = true;
    dashboardShell.hidden = false;
    byId("sign-out").hidden = false;
    byId("session-label").textContent = `Signed in as ${state.adminName}`;
    await refreshDashboard();
  } catch (error) {
    endSession("Signed out");
    if (error?.name !== "AbortError") {
      setError(error);
      setStatus("Sign-in failed.");
    }
  } finally {
    endBusy("login", button);
  }
}
function signOut() {
  const token = state.token;
  endSession("Signed out");
  const generation = state.generation;
  clearError();
  setStatus("Signed out. Tokens and one-time keys were cleared.");
  if (token) {
    void logoutRequest(token, generation)
      .catch((error) => {
        if (error?.name === "AbortError") {
          return;
        }
        setStatus(`Signed out locally; server logout failed: ${error.message}`);
      });
  }
}
async function createKey(event) {
  event.preventDefault();
  const button = event.submitter;
  if (!beginBusy("create-key", button)) {
    return;
  }
  const session = captureSession();
  clearError();
  try {
    const teamId = byId("key-team").value;
    const payload = {
      name: byId("key-name").value.trim(),
      description: byId("key-description").value.trim() || null,
      permissions: {
        allowed_models: splitScope(byId("key-models").value, "Allowed models"),
        allowed_endpoints: splitScope(
          byId("key-endpoints").value,
          "Allowed endpoints",
        ),
        max_tokens_per_request: null,
        is_admin: false,
        custom_permissions: [],
      },
      ...(teamId
        ? { team_id: teamId }
        : { user_id: state.adminId }),
    };
    const data = await apiRequest(
      "/v1/keys",
      { method: "POST", body: JSON.stringify(payload) },
      session,
    );
    ensureCurrent(session);
    showRawKey(data?.key);
    event.currentTarget.reset();
    await loadKeys(session);
    setStatus("API key created.");
  } catch (error) {
    if (error?.name !== "AbortError") {
      setError(error);
      setStatus("API key creation failed.");
    }
  } finally {
    endBusy("create-key", button);
  }
}
async function revokeKey(key, button) {
  if (!window.confirm(`Revoke API key “${key.name}”?`)) {
    return;
  }
  const busyKey = `revoke-key:${key.id}`;
  if (!beginBusy(busyKey, button)) {
    return;
  }
  const session = captureSession();
  clearError();
  try {
    await apiRequest(
      `/v1/keys/${encodeURIComponent(key.id)}`,
      { method: "DELETE" },
      session,
    );
    ensureCurrent(session);
    await loadKeys(session);
    setStatus("API key revoked.");
  } catch (error) {
    if (error?.name !== "AbortError") {
      setError(error);
      setStatus("API key revocation failed.");
    }
  } finally {
    endBusy(busyKey, button);
  }
}
async function createTeam(event) {
  event.preventDefault();
  const button = event.submitter;
  if (!beginBusy("create-team", button)) {
    return;
  }
  const session = captureSession();
  clearError();
  try {
    const payload = {
      name: byId("team-name").value.trim(),
      display_name: byId("team-display-name").value.trim() || null,
      description: byId("team-description").value.trim() || null,
    };
    await apiRequest(
      "/v1/teams",
      { method: "POST", body: JSON.stringify(payload) },
      session,
    );
    ensureCurrent(session);
    event.currentTarget.reset();
    await loadTeams(session);
    await loadTeamUsage(session);
    setStatus("Team created.");
  } catch (error) {
    if (error?.name !== "AbortError") {
      setError(error);
      setStatus("Team creation failed.");
    }
  } finally {
    endBusy("create-team", button);
  }
}
async function deleteTeam(team, button) {
  if (!window.confirm(`Delete team “${team.display_name || team.name}”?`)) {
    return;
  }
  const busyKey = `delete-team:${team.id}`;
  if (!beginBusy(busyKey, button)) {
    return;
  }
  const session = captureSession();
  clearError();
  try {
    await apiRequest(
      `/v1/teams/${encodeURIComponent(team.id)}`,
      { method: "DELETE" },
      session,
    );
    ensureCurrent(session);
    await loadTeams(session);
    await loadTeamUsage(session);
    setStatus("Team deleted.");
  } catch (error) {
    if (error?.name !== "AbortError") {
      setError(error);
      setStatus("Team deletion failed.");
    }
  } finally {
    endBusy(busyKey, button);
  }
}
function showRawKey(value) {
  if (!value) {
    throw new Error("Gateway did not return the new raw key.");
  }
  clearRawKey();
  state.rawKey = String(value);
  byId("raw-key-value").textContent = state.rawKey;
  rawKeyDialog.showModal();
}
function clearRawKey() {
  state.rawKey = null;
  byId("raw-key-value").textContent = "";
  if (rawKeyDialog.open) {
    rawKeyDialog.close();
  }
}
async function copyRawKey() {
  if (!state.rawKey) {
    return;
  }
  if (!navigator.clipboard?.writeText) {
    setError("Clipboard access is unavailable. Select and copy the key manually.");
    return;
  }
  try {
    await navigator.clipboard.writeText(state.rawKey);
    setStatus("Raw key copied. Store it securely.");
  } catch (error) {
    setError(`Could not copy the key: ${error.message}`);
  }
}
function showView(view) {
  state.currentView = view;
  for (const button of document.querySelectorAll("[data-view]")) {
    const active = button.dataset.view === view;
    button.classList.toggle("active", active);
    button.setAttribute("aria-selected", String(active));
  }
  for (const panel of ["keys", "teams", "spend", "health"]) {
    byId(`${panel}-panel`).hidden = panel !== view;
  }
  if (view === "spend") {
    void refreshDashboard();
  }
}
byId("login-form").addEventListener("submit", (event) => void signIn(event));
byId("sign-out").addEventListener("click", signOut);
byId("create-key-form").addEventListener("submit", (event) => void createKey(event));
byId("create-team-form").addEventListener("submit", (event) => void createTeam(event));
byId("refresh-keys").addEventListener("click", () => {
  setStatus("Loading API keys…");
  void loadKeys()
    .then(() => setStatus("API keys refreshed."))
    .catch((error) => reportRequestError(error, "API key refresh failed."));
});
byId("refresh-teams").addEventListener("click", () => {
  setStatus("Loading teams…");
  void loadTeams()
    .then(() => loadTeamUsage())
    .then(() => setStatus("Teams refreshed."))
    .catch((error) => reportRequestError(error, "Team refresh failed."));
});
byId("refresh-spend").addEventListener("click", () => void refreshDashboard());
byId("keys-previous").addEventListener("click", () => {
  state.keyPage = Math.max(1, state.keyPage - 1);
  setStatus("Loading the previous API key page…");
  void loadKeys().catch((error) =>
    reportRequestError(error, "API key page failed to load."),
  );
});
byId("keys-next").addEventListener("click", () => {
  state.keyPage += 1;
  setStatus("Loading the next API key page…");
  void loadKeys().catch((error) =>
    reportRequestError(error, "API key page failed to load."),
  );
});
byId("teams-previous").addEventListener("click", () => {
  state.teamPage = Math.max(1, state.teamPage - 1);
  setStatus("Loading the previous team page…");
  void loadTeams()
    .then(() => loadTeamUsage())
    .catch((error) => reportRequestError(error, "Team page failed to load."));
});
byId("teams-next").addEventListener("click", () => {
  state.teamPage += 1;
  setStatus("Loading the next team page…");
  void loadTeams()
    .then(() => loadTeamUsage())
    .catch((error) => reportRequestError(error, "Team page failed to load."));
});
byId("copy-raw-key").addEventListener("click", () => void copyRawKey());
byId("dismiss-raw-key").addEventListener("click", clearRawKey);
rawKeyDialog.addEventListener("close", clearRawKey);
for (const button of document.querySelectorAll("[data-view]")) {
  button.addEventListener("click", () => showView(button.dataset.view));
}
endSession();
