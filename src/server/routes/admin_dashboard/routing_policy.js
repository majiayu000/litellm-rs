"use strict";

window.createRoutingPolicyView = function createRoutingPolicyView({
  apiRequest,
  beginBusy,
  byId,
  captureSession,
  clearError,
  createAction,
  endBusy,
  ensureCurrent,
  reportRequestError,
  setStatus,
}) {
  const STRATEGIES = [
    "simple_shuffle",
    "least_busy",
    "usage_based",
    "latency_based",
    "priority_based",
    "rate_limit_aware",
    "round_robin",
  ];
  const RUNTIME_NOTICE = "The previous runtime revision is still active.";
  let activeGeneration = null;
  let knownProviders = [];
  let requestVersion = 0;
  let loaded = false;
  let loadPromise = null;

  function setNotice(message) {
    byId("routing-policy-notice").textContent = message || "";
  }

  function setGeneration(generation) {
    activeGeneration = generation;
    const label =
      generation == null || generation === "" ? "—" : String(generation);
    byId("routing-policy-generation").textContent = `Active revision: ${label}`;
  }

  function integerValue(raw, label) {
    const value = Number(raw);
    if (!Number.isInteger(value) || value < 0) {
      throw new Error(`${label} must be a non-negative integer.`);
    }
    return value;
  }

  function finiteValue(raw, label) {
    const value = Number(raw);
    if (!Number.isFinite(value)) {
      throw new Error(`${label} must be a finite number.`);
    }
    return value;
  }

  function addAliasRow(alias = {}) {
    const row = document.createElement("tr");
    const nameCell = document.createElement("td");
    const nameInput = document.createElement("input");
    nameInput.type = "text";
    nameInput.maxLength = 240;
    nameInput.value = alias.name || "";
    nameInput.setAttribute("aria-label", "Alias name");
    nameCell.append(nameInput);
    const targetCell = document.createElement("td");
    const targetInput = document.createElement("input");
    targetInput.type = "text";
    targetInput.maxLength = 240;
    targetInput.value = alias.target || "";
    targetInput.setAttribute("aria-label", "Alias target");
    targetCell.append(targetInput);
    const actionCell = document.createElement("td");
    actionCell.className = "provider-actions";
    actionCell.append(
      createAction("Remove", "secondary compact", () => {
        row.remove();
      }),
    );
    row.append(nameCell, targetCell, actionCell);
    byId("routing-policy-aliases-body").append(row);
  }

  function renderAliases(aliases) {
    byId("routing-policy-aliases-body").replaceChildren();
    const entries = Object.entries(aliases || {}).sort(([left], [right]) =>
      left.localeCompare(right),
    );
    for (const [name, target] of entries) {
      addAliasRow({ name, target: String(target || "") });
    }
  }

  function renderProviders(providers) {
    const names = Object.keys(providers || {}).sort();
    knownProviders = names;
    byId("routing-policy-providers-body").replaceChildren(
      ...names.map((name) => {
        const record = providers[name] || {};
        const row = document.createElement("tr");
        row.dataset.providerName = name;
        const nameCell = document.createElement("td");
        nameCell.textContent = name;
        const weightCell = document.createElement("td");
        const weightInput = document.createElement("input");
        weightInput.type = "number";
        weightInput.step = "any";
        weightInput.value = record.weight == null ? "" : String(record.weight);
        weightInput.setAttribute("aria-label", `${name} weight`);
        weightInput.dataset.field = "weight";
        weightCell.append(weightInput);
        const priorityCell = document.createElement("td");
        const priorityInput = document.createElement("input");
        priorityInput.type = "number";
        priorityInput.step = "1";
        priorityInput.value =
          record.priority == null ? "" : String(record.priority);
        priorityInput.setAttribute("aria-label", `${name} priority`);
        priorityInput.dataset.field = "priority";
        priorityCell.append(priorityInput);
        row.append(nameCell, weightCell, priorityCell);
        return row;
      }),
    );
    byId("routing-policy-providers-empty").hidden = names.length !== 0;
  }

  function fillForm(policy) {
    const strategy = String(policy.strategy || "");
    const select = byId("routing-policy-strategy");
    if (STRATEGIES.includes(strategy)) {
      select.value = strategy;
    }
    const breaker = policy.circuit_breaker || {};
    byId("routing-policy-failure-threshold").value =
      breaker.failure_threshold == null ? "" : String(breaker.failure_threshold);
    byId("routing-policy-recovery-timeout").value =
      breaker.recovery_timeout == null ? "" : String(breaker.recovery_timeout);
    byId("routing-policy-min-requests").value =
      breaker.min_requests == null ? "" : String(breaker.min_requests);
    byId("routing-policy-success-threshold").value =
      breaker.success_threshold == null ? "" : String(breaker.success_threshold);
    const balancer = policy.load_balancer || {};
    byId("routing-policy-health-check-enabled").checked =
      balancer.health_check_enabled !== false;
    byId("routing-policy-sticky-sessions").checked = Boolean(
      balancer.sticky_sessions,
    );
    byId("routing-policy-session-timeout").value =
      balancer.session_timeout == null ? "" : String(balancer.session_timeout);
    renderAliases(policy.model_aliases);
    renderProviders(policy.providers);
  }

  function collectAliases() {
    const aliases = {};
    for (const row of byId("routing-policy-aliases-body").querySelectorAll("tr")) {
      const inputs = row.querySelectorAll("input");
      const name = String(inputs[0]?.value || "").trim();
      const target = String(inputs[1]?.value || "").trim();
      if (!name && !target) {
        continue;
      }
      if (!name || !target) {
        throw new Error("Each alias needs both a name and a target.");
      }
      if (Object.hasOwn(aliases, name)) {
        throw new Error(`Duplicate alias '${name}'.`);
      }
      aliases[name] = target;
    }
    return aliases;
  }

  function collectProviders() {
    if (!knownProviders.length) {
      return null;
    }
    const rows = [...byId("routing-policy-providers-body").querySelectorAll("tr")];
    const providers = {};
    for (const name of knownProviders) {
      const row = rows.find((entry) => entry.dataset.providerName === name);
      if (!row) {
        throw new Error(`Provider '${name}' is missing from the editor.`);
      }
      providers[name] = {
        weight: finiteValue(
          row.querySelector('input[data-field="weight"]')?.value,
          `${name} weight`,
        ),
        priority: integerValue(
          row.querySelector('input[data-field="priority"]')?.value,
          `${name} priority`,
        ),
      };
    }
    return providers;
  }

  function buildPayload() {
    const strategy = byId("routing-policy-strategy").value;
    if (!STRATEGIES.includes(strategy)) {
      throw new Error("Choose a supported routing strategy.");
    }
    const payload = {
      strategy,
      circuit_breaker: {
        failure_threshold: integerValue(
          byId("routing-policy-failure-threshold").value,
          "Failure threshold",
        ),
        recovery_timeout: integerValue(
          byId("routing-policy-recovery-timeout").value,
          "Recovery timeout",
        ),
        min_requests: integerValue(
          byId("routing-policy-min-requests").value,
          "Minimum requests",
        ),
        success_threshold: integerValue(
          byId("routing-policy-success-threshold").value,
          "Success threshold",
        ),
      },
      load_balancer: {
        health_check_enabled: byId("routing-policy-health-check-enabled")
          .checked,
        sticky_sessions: byId("routing-policy-sticky-sessions").checked,
        session_timeout: integerValue(
          byId("routing-policy-session-timeout").value,
          "Session timeout",
        ),
      },
      model_aliases: collectAliases(),
    };
    const providers = collectProviders();
    if (providers) {
      payload.providers = providers;
    }
    return payload;
  }

  function applyPolicy(data) {
    if (data?.generation == null || typeof data.policy !== "object" || !data.policy) {
      throw new Error("Routing policy response did not include generation and policy.");
    }
    setGeneration(data.generation);
    fillForm(data.policy);
  }

  function clearDraft() {
    knownProviders = [];
    byId("routing-policy-form").reset();
    byId("routing-policy-aliases-body").replaceChildren();
    byId("routing-policy-providers-body").replaceChildren();
    byId("routing-policy-providers-empty").hidden = false;
    setGeneration(null);
    setNotice("");
  }

  function reset() {
    requestVersion += 1;
    loaded = false;
    loadPromise = null;
    byId("refresh-routing-policy").disabled = false;
    clearDraft();
  }

  async function load(session = captureSession()) {
    const version = ++requestVersion;
    const data = await apiRequest("/admin/routing/policy", {}, session);
    ensureCurrent(session);
    if (version !== requestVersion) {
      throw new DOMException("Stale routing policy response", "AbortError");
    }
    applyPolicy(data);
  }

  function loadOnce() {
    if (loaded || loadPromise) {
      return loadPromise || Promise.resolve();
    }
    clearError();
    setStatus("Loading routing policy…");
    const pending = load()
      .then(() => {
        loaded = true;
        setStatus("Routing policy loaded.");
      })
      .catch((error) => reportRequestError(error, "Routing policy load failed."));
    loadPromise = pending;
    void pending.finally(() => {
      if (loadPromise === pending) {
        loadPromise = null;
      }
    });
    return pending;
  }

  function reportApplyFailure(error, statusMessage) {
    const detail = error?.message || "Unknown request failure";
    setNotice(`${RUNTIME_NOTICE} ${detail}`);
    reportRequestError(error, `${statusMessage} ${RUNTIME_NOTICE}`);
  }

  async function save(event) {
    event.preventDefault();
    const button = event.submitter;
    if (!beginBusy("save-routing-policy", button)) {
      return;
    }
    const session = captureSession();
    const version = requestVersion;
    clearError();
    setNotice("");
    try {
      const payload = buildPayload();
      const data = await apiRequest(
        "/admin/routing/policy",
        { method: "PUT", body: JSON.stringify(payload) },
        session,
      );
      ensureCurrent(session);
      if (version !== requestVersion) {
        throw new DOMException("Stale routing policy response", "AbortError");
      }
      applyPolicy(data);
      setStatus("Routing policy updated.");
    } catch (error) {
      if (error?.name !== "AbortError") {
        reportApplyFailure(error, "Routing policy update failed.");
      }
    } finally {
      endBusy("save-routing-policy", button);
    }
  }

  async function refresh(button) {
    if (button.disabled) {
      return;
    }
    button.disabled = true;
    clearError();
    setNotice("");
    setStatus("Refreshing routing policy…");
    try {
      await load();
      setStatus("Routing policy refreshed.");
    } catch (error) {
      reportRequestError(error, "Routing policy refresh failed.");
    } finally {
      button.disabled = false;
    }
  }

  byId("routing-policy-form").addEventListener("submit", (event) =>
    void save(event),
  );
  byId("routing-policy-add-alias").addEventListener("click", () => addAliasRow());
  byId("refresh-routing-policy").addEventListener("click", (event) =>
    void refresh(event.currentTarget),
  );

  return { load, loadOnce, reset };
};
