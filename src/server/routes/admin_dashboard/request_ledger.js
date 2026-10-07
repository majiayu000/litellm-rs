"use strict";

window.createRequestLedgerView = function createRequestLedgerView({
  apiRequest,
  byId,
  captureSession,
  clearError,
  createAction,
  ensureCurrent,
  reportRequestError,
  setStatus,
  textCell,
}) {
  let items = [];
  let nextCursor = null;
  let hasMore = false;
  let currentCursor = null;
  let cursorStack = [];
  let selectedId = null;
  let requestError = null;
  let requestVersion = 0;
  let loaded = false;
  let loadPromise = null;

  function labeledId(kind, value) {
    if (value == null || value === "") {
      return "—";
    }
    return `${kind} ${value}`;
  }

  function formatDateTime(value) {
    if (!value) {
      return "—";
    }
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? String(value) : date.toLocaleString();
  }

  function formatMoney(value) {
    if (value == null || value === "") {
      return "—";
    }
    const number = typeof value === "number" ? value : Number(value);
    return Number.isFinite(number) ? `$${number.toFixed(4)}` : "—";
  }

  function toRfc3339(value) {
    const trimmed = String(value || "").trim();
    if (!trimmed) {
      return "";
    }
    const date = new Date(trimmed);
    return Number.isNaN(date.getTime()) ? trimmed : date.toISOString();
  }

  function currentFilters() {
    return {
      finished_after: toRfc3339(byId("request-logs-after").value),
      finished_before: toRfc3339(byId("request-logs-before").value),
      request_id: byId("request-logs-request-id").value.trim(),
      model: byId("request-logs-model").value.trim(),
      provider: byId("request-logs-provider").value.trim(),
      terminal_status: byId("request-logs-status").value,
    };
  }

  function buildPath(cursor) {
    const params = new URLSearchParams();
    const filters = currentFilters();
    for (const [key, value] of Object.entries(filters)) {
      if (value) {
        params.set(key, value);
      }
    }
    if (cursor) {
      params.set("cursor", cursor);
    }
    const query = params.toString();
    return query ? `/admin/request-ledger?${query}` : "/admin/request-ledger";
  }

  function selectedItem() {
    return items.find((item) => item.request_id === selectedId) || null;
  }

  function renderDetail() {
    const detail = byId("request-logs-detail");
    const item = selectedItem();
    detail.replaceChildren();
    if (!item) {
      detail.hidden = true;
      return;
    }
    detail.hidden = false;
    const title = document.createElement("h3");
    title.textContent = "Request summary";
    const list = document.createElement("dl");
    const fields = [
      ["Request ID", item.request_id],
      ["Started", formatDateTime(item.started_at)],
      ["Finished", formatDateTime(item.finished_at)],
      ["Method", item.method || "—"],
      ["Endpoint", item.endpoint || "—"],
      ["Model", item.model || "—"],
      ["Provider", item.provider || "—"],
      ["Deployment", item.deployment || "—"],
      ["Status code", item.status_code == null ? "—" : String(item.status_code)],
      ["Terminal status", item.terminal_status || "—"],
      ["Latency", item.latency_ms == null ? "—" : `${item.latency_ms} ms`],
      ["Prompt tokens", item.prompt_tokens == null ? "—" : String(item.prompt_tokens)],
      ["Completion tokens", item.completion_tokens == null ? "—" : String(item.completion_tokens)],
      ["Total tokens", item.total_tokens == null ? "—" : String(item.total_tokens)],
      ["Gateway priced cost", formatMoney(item.cost)],
      ["Charge basis", item.billing?.charge_basis || "Not recorded"],
      ["Known Realtime subtotal", formatMoney(item.billing?.known_cost_subtotal)],
      ["Unknown Realtime responses", item.billing?.unknown_response_count == null ? "—" : String(item.billing.unknown_response_count)],
      ["Pending Realtime responses", item.billing?.pending_response_count == null ? "—" : String(item.billing.pending_response_count)],
      ["Unknown reason", item.billing?.unknown_reason || "—"],
      ["Awaiting reconciliation", item.awaiting_duration_ms == null ? "—" : `${Math.floor(item.awaiting_duration_ms / 1000)} s`],
      ["Provider budget reserved", formatMoney(item.billing?.provider_reserved_amount)],
      ["Model budget reserved", formatMoney(item.billing?.model_reserved_amount)],
      ["Key budget reserved", formatMoney(item.billing?.key_reserved_amount)],
      ["Latest reservation", formatDateTime(item.billing?.reserved_at)],
      ["Last provider lease identity", item.billing?.provider_lease_id || "—"],
      ["Last model lease identity", item.billing?.model_lease_id || "—"],
      ["Provider budget state", item.billing?.provider_settlement || "Not recorded"],
      ["Model budget state", item.billing?.model_settlement || "Not recorded"],
      ["Key budget state", item.billing?.key_settlement || "Not recorded"],
      ["Provider budget charged", formatMoney(item.billing?.provider_charge_amount)],
      ["Model budget charged", formatMoney(item.billing?.model_charge_amount)],
      ["Key budget charged", formatMoney(item.billing?.key_charge_amount)],
      ["Supplier verified cost", formatMoney(item.reconciliation?.verified_actual_cost)],
      ["Supplier evidence reference", item.reconciliation?.evidence_reference || "—"],
      ["Verified at", formatDateTime(item.reconciliation?.verified_at)],
      ["Reconciliation", item.reconciliation ? (item.reconciliation.budget_review_required ? "Supplier verified; budget reconciliation / manual review required" : "Supplier verified; budget charges match") : (item.billing?.unknown_reason ? "Awaiting supplier verification" : "—")],
      ["User", labeledId("User", item.user_id)],
      ["API key", labeledId("Key", item.api_key_id)],
      ["Team", labeledId("Team", item.team_id)],
    ];
    for (const [label, value] of fields) {
      const term = document.createElement("dt");
      term.textContent = label;
      const definition = document.createElement("dd");
      definition.textContent = value == null ? "—" : String(value);
      list.append(term, definition);
    }
    detail.append(title, list);
    const note = document.createElement("p");
    note.textContent = "Budget scopes enforce the same request: do not add their reserved or charged amounts. Supplier verification records evidence and does not adjust budget counters.";
    detail.append(note);
    if (item.billing?.unknown_reason && !item.reconciliation) {
      const form = document.createElement("form");
      const amount = document.createElement("input");
      amount.type = "number";
      amount.min = "0";
      amount.step = "any";
      amount.required = true;
      amount.setAttribute("aria-label", "Supplier verified cost");
      amount.placeholder = "Supplier verified cost";
      const evidence = document.createElement("input");
      evidence.required = true;
      evidence.maxLength = 512;
      evidence.setAttribute("aria-label", "Invoice or receipt reference (no secrets)");
      evidence.placeholder = "Invoice / receipt reference (no secrets)";
      const submit = document.createElement("button");
      submit.type = "submit";
      submit.textContent = "Record supplier verification";
      form.append(amount, evidence, submit);
      form.addEventListener("submit", async (event) => {
        event.preventDefault();
        submit.disabled = true;
        const session = captureSession();
        clearError();
        try {
          await apiRequest(`/admin/request-ledger/${encodeURIComponent(item.request_id)}/reconciliation`, {
            method: "POST",
            body: JSON.stringify({ verified_actual_cost: Number(amount.value), evidence_reference: evidence.value }),
          }, session);
          ensureCurrent(session);
          await load(session);
          setStatus("Supplier verification recorded; budget counters were not adjusted.");
        } catch (error) {
          reportRequestError(error, "Supplier verification failed.");
        } finally {
          submit.disabled = false;
        }
      });
      detail.append(form);
    }
  }

  function render() {
    const rows = items.map((item) => {
      const row = document.createElement("tr");
      if (item.request_id === selectedId) {
        row.dataset.selected = "true";
      }
      const action = document.createElement("td");
      action.append(
        createAction("View", "secondary", () => {
          selectedId = item.request_id;
          render();
        }),
      );
      row.append(
        textCell(item.request_id),
        textCell(formatDateTime(item.finished_at)),
        textCell(item.model || "—"),
        textCell(item.provider || "—"),
        textCell(item.terminal_status || "—"),
        textCell(item.status_code == null ? "—" : String(item.status_code)),
        textCell(item.latency_ms == null ? "—" : `${item.latency_ms} ms`),
        textCell(formatMoney(item.cost)),
        textCell(labeledId("Key", item.api_key_id)),
        action,
      );
      return row;
    });
    byId("request-logs-body").replaceChildren(...rows);
    byId("request-logs-empty").hidden = items.length !== 0 || Boolean(requestError);
    byId("request-logs-notice").textContent = requestError
      ? `Request logs unavailable: ${requestError}`
      : "";
    byId("request-logs-summary").textContent = requestError
      ? ""
      : loaded
        ? `${items.length} request${items.length === 1 ? "" : "s"} on this page`
        : "";
    byId("request-logs-previous").disabled = cursorStack.length === 0;
    byId("request-logs-next").disabled = !hasMore;
    renderDetail();
  }

  function reset() {
    requestVersion += 1;
    loaded = false;
    loadPromise = null;
    items = [];
    nextCursor = null;
    hasMore = false;
    currentCursor = null;
    cursorStack = [];
    selectedId = null;
    requestError = null;
    byId("request-logs-filter").reset();
    byId("refresh-request-logs").disabled = false;
    render();
  }

  async function load(session = captureSession()) {
    const version = ++requestVersion;
    try {
      const data = await apiRequest(buildPath(currentCursor), {}, session);
      ensureCurrent(session);
      if (version !== requestVersion) {
        throw new DOMException("Stale request log response", "AbortError");
      }
      if (!Array.isArray(data?.items)) {
        throw new Error("Request ledger response did not include items.");
      }
      items = data.items;
      nextCursor = data.next_cursor || null;
      hasMore = Boolean(data.has_more);
      if (selectedId && !items.some((item) => item.request_id === selectedId)) {
        selectedId = null;
      }
      requestError = null;
      loaded = true;
      render();
    } catch (error) {
      if (error?.name !== "AbortError") {
        ensureCurrent(session);
        if (version === requestVersion) {
          items = [];
          nextCursor = null;
          hasMore = false;
          selectedId = null;
          requestError = error.message || "Unknown request failure";
          loaded = true;
          render();
        }
      }
      throw error;
    }
  }

  function loadOnce() {
    if (loaded || loadPromise) {
      return loadPromise || Promise.resolve();
    }
    clearError();
    setStatus("Loading request logs…");
    const pending = load()
      .then(() => setStatus("Request logs loaded."))
      .catch((error) => reportRequestError(error, "Request log load failed."));
    loadPromise = pending;
    void pending.finally(() => {
      if (loadPromise === pending) {
        loadPromise = null;
      }
    });
    return pending;
  }

  async function refresh(button) {
    if (button.disabled) {
      return;
    }
    button.disabled = true;
    clearError();
    setStatus("Refreshing request logs…");
    try {
      await load();
      setStatus("Request logs refreshed.");
    } catch (error) {
      reportRequestError(error, "Request log refresh failed.");
    } finally {
      button.disabled = false;
    }
  }

  async function applyFilters(event) {
    event.preventDefault();
    cursorStack = [];
    currentCursor = null;
    selectedId = null;
    clearError();
    setStatus("Filtering request logs…");
    try {
      await load();
      setStatus("Request logs filtered.");
    } catch (error) {
      reportRequestError(error, "Request log filter failed.");
    }
  }

  async function goNext() {
    if (!hasMore || !nextCursor) {
      return;
    }
    cursorStack.push(currentCursor);
    currentCursor = nextCursor;
    selectedId = null;
    clearError();
    setStatus("Loading the next request log page…");
    try {
      await load();
      setStatus("Request logs page loaded.");
    } catch (error) {
      currentCursor = cursorStack.pop() ?? null;
      reportRequestError(error, "Request log page failed to load.");
    }
  }

  async function goPrevious() {
    if (cursorStack.length === 0) {
      return;
    }
    currentCursor = cursorStack.pop() ?? null;
    selectedId = null;
    clearError();
    setStatus("Loading the previous request log page…");
    try {
      await load();
      setStatus("Request logs page loaded.");
    } catch (error) {
      reportRequestError(error, "Request log page failed to load.");
    }
  }

  byId("refresh-request-logs").addEventListener("click", (event) =>
    void refresh(event.currentTarget),
  );
  byId("request-logs-filter").addEventListener("submit", (event) =>
    void applyFilters(event),
  );
  byId("request-logs-next").addEventListener("click", () => void goNext());
  byId("request-logs-previous").addEventListener("click", () => void goPrevious());

  return { load, loadOnce, reset };
};
