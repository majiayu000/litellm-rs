"use strict";

window.createBudgetView = function createBudgetView({
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
  textCell,
}) {
  let providers = [];
  let models = [];
  let requestVersion = 0;
  let loaded = false;
  let loadPromise = null;
  let editingKey = null;

  const scopeConfig = {
    provider: {
      collection: "providers",
      field: "provider",
      label: "Provider",
      rows: () => providers,
    },
    model: {
      collection: "models",
      field: "model",
      label: "Model",
      rows: () => models,
    },
  };

  function configFor(scope) {
    const config = scopeConfig[scope];
    if (!config) {
      throw new Error("Choose a provider or model budget scope.");
    }
    return config;
  }

  function budgetName(scope, budget) {
    return budget[configFor(scope).field];
  }

  function formatAmount(value, currency) {
    const amount = Number(value);
    const unit = String(currency || "USD").toUpperCase();
    if (!Number.isFinite(amount)) {
      return "";
    }
    try {
      return new Intl.NumberFormat(undefined, {
        style: "currency",
        currency: unit,
        minimumFractionDigits: 2,
        maximumFractionDigits: 4,
      }).format(amount);
    } catch {
      return `${unit} ${amount.toFixed(4)}`;
    }
  }

  function statusCell(status, enabled) {
    let value = "disabled";
    if (enabled) {
      value = ["ok", "warning", "exceeded"].includes(status)
        ? status
        : "unknown";
    }
    const cell = document.createElement("td");
    const badge = document.createElement("span");
    badge.className = "budget-state";
    badge.dataset.budgetState = value;
    badge.textContent = value;
    cell.append(badge);
    return cell;
  }

  function actionsCell(scope, budget) {
    const name = budgetName(scope, budget);
    const cell = document.createElement("td");
    cell.className = "budget-actions";
    cell.append(
      createAction("Edit", "secondary compact", () => editBudget(scope, budget)),
      createAction("Reset", "secondary compact", (event) => {
        void resetBudget(scope, budget, event.currentTarget);
      }),
      createAction("Delete", "danger compact", (event) => {
        void deleteBudget(scope, budget, event.currentTarget);
      }),
    );
    cell.setAttribute("aria-label", `${configFor(scope).label} ${name} actions`);
    return cell;
  }

  function renderRows(scope) {
    const config = configFor(scope);
    const rows = config.rows();
    byId(`${scope}-budgets-body`).replaceChildren(
      ...rows.map((budget) => {
        const row = document.createElement("tr");
        row.append(
          textCell(budgetName(scope, budget)),
          textCell(formatAmount(budget.current_spend, budget.currency)),
          textCell(formatAmount(budget.remaining, budget.currency)),
          statusCell(
            String(budget.status || "").toLowerCase(),
            budget.enabled !== false,
          ),
          textCell(budget.reset_period),
          actionsCell(scope, budget),
        );
        return row;
      }),
    );
    byId(`${scope}-budgets-empty`).hidden = rows.length !== 0;
  }

  function render() {
    renderRows("provider");
    renderRows("model");
  }

  function reset() {
    requestVersion += 1;
    loaded = false;
    loadPromise = null;
    providers = [];
    models = [];
    clearEditor();
    render();
  }

  async function load(session = captureSession()) {
    const version = ++requestVersion;
    const [providerData, modelData] = await Promise.all([
      apiRequest("/v1/budget/providers", {}, session),
      apiRequest("/v1/budget/models", {}, session),
    ]);
    ensureCurrent(session);
    if (version !== requestVersion) {
      throw new DOMException("Stale budget response", "AbortError");
    }
    if (!Array.isArray(providerData?.providers)) {
      throw new Error("Provider budget response did not include a list.");
    }
    if (!Array.isArray(modelData?.models)) {
      throw new Error("Model budget response did not include a list.");
    }
    providers = providerData.providers;
    models = modelData.models;
    render();
  }

  function loadOnce() {
    if (loaded || loadPromise) {
      return loadPromise || Promise.resolve();
    }
    clearError();
    setStatus("Loading budgets…");
    const pending = load()
      .then(() => {
        loaded = true;
        setStatus("Budgets loaded.");
      })
      .catch((error) => reportRequestError(error, "Budget load failed."));
    loadPromise = pending;
    void pending.finally(() => {
      if (loadPromise === pending) {
        loadPromise = null;
      }
    });
    return pending;
  }

  async function refreshAfterMutation(session, successMessage) {
    try {
      await load(session);
      setStatus(successMessage);
    } catch (error) {
      if (error?.name === "AbortError") {
        throw error;
      }
      reportRequestError(
        new Error(`${successMessage} Budget list refresh failed: ${error.message}`),
        `${successMessage} Budget list refresh failed.`,
      );
    }
  }

  function updateScopeLabel() {
    const scope = byId("budget-scope").value;
    const config = configFor(scope);
    byId("budget-name-label").textContent = `${config.label} name`;
    byId("budget-name").placeholder = scope === "provider" ? "openai" : "gpt-4o";
  }

  function setEditMode(editing) {
    byId("budget-scope").disabled = editing;
    byId("budget-name").readOnly = editing;
    byId("cancel-budget-edit").hidden = !editing;
  }

  function setCurrency(currency) {
    const select = byId("budget-currency");
    select.querySelector("[data-preserved-currency]")?.remove();
    const value = String(currency || "USD").toUpperCase();
    if (![...select.options].some((option) => option.value === value)) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = value;
      option.dataset.preservedCurrency = "true";
      select.append(option);
    }
    select.value = value;
  }

  function clearEditor() {
    editingKey = null;
    byId("budget-currency").querySelector("[data-preserved-currency]")?.remove();
    byId("budget-form").reset();
    setEditMode(false);
    updateScopeLabel();
  }

  function editBudget(scope, budget) {
    const form = byId("budget-form");
    editingKey = { scope, name: budgetName(scope, budget) };
    byId("budget-scope").value = scope;
    updateScopeLabel();
    byId("budget-name").value = budgetName(scope, budget);
    byId("budget-max").value = budget.max_budget;
    byId("budget-reset-period").value = budget.reset_period;
    setCurrency(budget.currency);
    byId("budget-enabled").checked = budget.enabled !== false;
    setEditMode(true);
    byId("budget-editor").open = true;
    form.scrollIntoView?.({ behavior: "smooth", block: "nearest" });
    byId("budget-max").focus();
  }

  async function saveBudget(event) {
    event.preventDefault();
    const form = event.currentTarget;
    const button = event.submitter;
    if (!beginBusy("save-budget", button)) {
      return;
    }
    const session = captureSession();
    clearError();
    try {
      const scope = editingKey?.scope || byId("budget-scope").value;
      const config = configFor(scope);
      const name = editingKey?.name || byId("budget-name").value.trim();
      const payload = {
        [config.field]: name,
        max_budget: Number(byId("budget-max").value),
        reset_period: byId("budget-reset-period").value,
        currency: byId("budget-currency").value,
        enabled: byId("budget-enabled").checked,
      };
      await apiRequest(
        `/v1/budget/${config.collection}`,
        { method: "POST", body: JSON.stringify(payload) },
        session,
      );
      ensureCurrent(session);
      clearEditor();
      await refreshAfterMutation(session, `${config.label} budget saved.`);
    } catch (error) {
      if (error?.name !== "AbortError") {
        reportRequestError(error, "Budget save failed.");
      }
    } finally {
      endBusy("save-budget", button);
    }
  }

  async function resetBudget(scope, budget, button) {
    const config = configFor(scope);
    const name = budgetName(scope, budget);
    if (!window.confirm(`Reset ${scope} budget “${name}” to zero spend?`)) {
      return;
    }
    const busyKey = `reset-${scope}-budget:${name}`;
    if (!beginBusy(busyKey, button)) {
      return;
    }
    const session = captureSession();
    clearError();
    try {
      await apiRequest(
        `/v1/budget/${config.collection}/${encodeURIComponent(name)}/reset`,
        { method: "POST" },
        session,
      );
      ensureCurrent(session);
      await refreshAfterMutation(session, `${config.label} budget reset.`);
    } catch (error) {
      reportRequestError(error, "Budget reset failed.");
    } finally {
      endBusy(busyKey, button);
    }
  }

  async function deleteBudget(scope, budget, button) {
    const config = configFor(scope);
    const name = budgetName(scope, budget);
    if (!window.confirm(`Delete ${scope} budget “${name}”?`)) {
      return;
    }
    const busyKey = `delete-${scope}-budget:${name}`;
    if (!beginBusy(busyKey, button)) {
      return;
    }
    const session = captureSession();
    clearError();
    try {
      await apiRequest(
        `/v1/budget/${config.collection}/${encodeURIComponent(name)}`,
        { method: "DELETE" },
        session,
      );
      ensureCurrent(session);
      await refreshAfterMutation(session, `${config.label} budget deleted.`);
    } catch (error) {
      reportRequestError(error, "Budget deletion failed.");
    } finally {
      endBusy(busyKey, button);
    }
  }

  async function refresh(button) {
    if (button.disabled) {
      return;
    }
    button.disabled = true;
    clearError();
    setStatus("Refreshing budgets…");
    try {
      await load();
      setStatus("Budgets refreshed.");
    } catch (error) {
      reportRequestError(error, "Budget refresh failed.");
    } finally {
      button.disabled = false;
    }
  }

  byId("budget-scope").addEventListener("change", updateScopeLabel);
  byId("budget-form").addEventListener("submit", (event) => void saveBudget(event));
  byId("cancel-budget-edit").addEventListener("click", clearEditor);
  byId("refresh-budgets").addEventListener("click", (event) =>
    void refresh(event.currentTarget),
  );
  updateScopeLabel();

  return { load, loadOnce, reset };
};
