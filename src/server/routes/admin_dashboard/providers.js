"use strict";

window.createProviderEditorView = function createProviderEditorView({
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
  const RUNTIME_NOTICE = "The previous runtime revision is still active.";
  let providers = [];
  let requestVersion = 0;
  let loaded = false;
  let loadPromise = null;
  let editingName = null;

  function csvList(value) {
    return String(value || "")
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean);
  }

  function formatList(values) {
    return Array.isArray(values) && values.length ? values.join(", ") : "";
  }

  function numberField(id, fallback) {
    const raw = byId(id).value;
    if (raw === "" || raw == null) {
      return fallback;
    }
    const value = Number(raw);
    if (!Number.isFinite(value)) {
      throw new Error("Weight and priority must be finite numbers.");
    }
    return value;
  }

  function sanitize(records) {
    return records.map((record) => ({
      name: record.name,
      provider_type: record.provider_type,
      base_url: record.base_url,
      enabled: record.enabled !== false,
      models: Array.isArray(record.models) ? record.models : [],
      tags: Array.isArray(record.tags) ? record.tags : [],
      weight: record.weight,
      priority: record.priority,
      api_key_ref: record.api_key_ref,
    }));
  }

  function setNotice(message) {
    byId("providers-notice").textContent = message || "";
  }

  function setEditorVisible(editing) {
    byId("provider-editor").hidden = !editing;
    byId("provider-editor").open = editing;
  }

  function secretRefText(provider) {
    if (provider?.api_key_ref) {
      return `Current reference: ${provider.api_key_ref}`;
    }
    return "No replacement reference on file.";
  }

  function clearCreateForm() {
    byId("create-provider-form").reset();
    byId("provider-create-enabled").checked = true;
  }

  function clearEditor() {
    editingName = null;
    byId("edit-provider-form").reset();
    byId("provider-edit-api-key").value = "";
    byId("provider-edit-api-key-ref").textContent = "";
    setEditorVisible(false);
  }

  function fillEditor(provider) {
    editingName = provider.name;
    byId("provider-edit-name").value = provider.name;
    byId("provider-edit-type").value = provider.provider_type || "";
    byId("provider-edit-api-key").value = "";
    byId("provider-edit-api-key-ref").textContent = secretRefText(provider);
    byId("provider-edit-base-url").value = provider.base_url || "";
    byId("provider-edit-models").value = formatList(provider.models);
    byId("provider-edit-tags").value = formatList(provider.tags);
    byId("provider-edit-weight").value =
      provider.weight == null ? "" : String(provider.weight);
    byId("provider-edit-priority").value =
      provider.priority == null ? "" : String(provider.priority);
    byId("provider-edit-enabled").checked = provider.enabled !== false;
    setEditorVisible(true);
    byId("provider-editor").scrollIntoView?.({
      behavior: "smooth",
      block: "nearest",
    });
    byId("provider-edit-type").focus();
  }

  function createPayload(prefix) {
    const payload = {
      name: byId(`${prefix}-name`).value.trim(),
      provider_type: byId(`${prefix}-type`).value.trim(),
      api_key: byId(`${prefix}-api-key`).value.trim(),
      base_url: byId(`${prefix}-base-url`).value.trim() || null,
      models: csvList(byId(`${prefix}-models`).value),
      tags: csvList(byId(`${prefix}-tags`).value),
      enabled: byId(`${prefix}-enabled`).checked,
      weight: numberField(`${prefix}-weight`, 1),
      priority: numberField(`${prefix}-priority`, 0),
    };
    if (!payload.name) {
      throw new Error("Provider name cannot be empty.");
    }
    if (!payload.provider_type) {
      throw new Error("Provider type cannot be empty.");
    }
    return payload;
  }

  function patchPayload() {
    const payload = createPayload("provider-edit");
    const patch = {
      provider_type: payload.provider_type,
      base_url: payload.base_url,
      models: payload.models,
      tags: payload.tags,
      enabled: payload.enabled,
      weight: payload.weight,
      priority: payload.priority,
    };
    if (payload.api_key) {
      patch.api_key = payload.api_key;
    }
    return patch;
  }

  function enabledLabel(enabled) {
    return enabled ? "enabled" : "disabled";
  }

  function actionsCell(provider) {
    const name = provider.name;
    const cell = document.createElement("td");
    cell.className = "provider-actions";
    cell.append(
      createAction("Edit", "secondary compact", () => fillEditor(provider)),
      createAction(
        provider.enabled ? "Disable" : "Enable",
        "secondary compact",
        (event) => {
          void toggleProvider(provider, event.currentTarget);
        },
      ),
      createAction("Delete", "danger compact", (event) => {
        void deleteProvider(provider, event.currentTarget);
      }),
    );
    cell.setAttribute("aria-label", `Provider ${name} actions`);
    return cell;
  }

  function render() {
    byId("providers-body").replaceChildren(
      ...providers.map((provider) => {
        const row = document.createElement("tr");
        row.append(
          textCell(provider.name),
          textCell(provider.provider_type),
          textCell(enabledLabel(provider.enabled)),
          textCell(formatList(provider.models)),
          textCell(provider.api_key_ref || "not set"),
          actionsCell(provider),
        );
        return row;
      }),
    );
    byId("providers-empty").hidden = providers.length !== 0;
  }

  function reset() {
    requestVersion += 1;
    loaded = false;
    loadPromise = null;
    providers = [];
    clearCreateForm();
    clearEditor();
    setNotice("");
    render();
  }

  async function load(session = captureSession()) {
    const version = ++requestVersion;
    const data = await apiRequest("/admin/providers", {}, session);
    ensureCurrent(session);
    if (version !== requestVersion) {
      throw new DOMException("Stale provider response", "AbortError");
    }
    if (!Array.isArray(data?.providers)) {
      throw new Error("Provider response did not include a list.");
    }
    providers = sanitize(data.providers);
    render();
  }

  function loadOnce() {
    if (loaded || loadPromise) {
      return loadPromise || Promise.resolve();
    }
    clearError();
    setStatus("Loading providers…");
    const pending = load()
      .then(() => {
        loaded = true;
        setStatus("Providers loaded.");
      })
      .catch((error) => reportRequestError(error, "Provider load failed."));
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
        new Error(
          `${successMessage} Provider list refresh failed: ${error.message}`,
        ),
        `${successMessage} Provider list refresh failed.`,
      );
    }
  }

  function reportApplyFailure(error, statusMessage) {
    const detail = error?.message || "Unknown request failure";
    setNotice(`${RUNTIME_NOTICE} ${detail}`);
    reportRequestError(error, `${statusMessage} ${RUNTIME_NOTICE}`);
  }

  async function createProvider(event) {
    event.preventDefault();
    const button = event.submitter;
    if (!beginBusy("create-provider", button)) {
      return;
    }
    const session = captureSession();
    clearError();
    setNotice("");
    try {
      const payload = createPayload("provider-create");
      await apiRequest(
        "/admin/providers",
        { method: "POST", body: JSON.stringify(payload) },
        session,
      );
      ensureCurrent(session);
      clearCreateForm();
      await refreshAfterMutation(session, "Provider created.");
    } catch (error) {
      if (error?.name !== "AbortError") {
        reportApplyFailure(error, "Provider create failed.");
      }
    } finally {
      endBusy("create-provider", button);
    }
  }

  async function saveProvider(event) {
    event.preventDefault();
    const button = event.submitter;
    if (!beginBusy("save-provider", button)) {
      return;
    }
    const session = captureSession();
    clearError();
    setNotice("");
    try {
      const name = editingName || byId("provider-edit-name").value.trim();
      await apiRequest(
        `/admin/providers/${encodeURIComponent(name)}`,
        { method: "PATCH", body: JSON.stringify(patchPayload()) },
        session,
      );
      ensureCurrent(session);
      clearEditor();
      await refreshAfterMutation(session, "Provider updated.");
    } catch (error) {
      if (error?.name !== "AbortError") {
        reportApplyFailure(error, "Provider update failed.");
      }
    } finally {
      endBusy("save-provider", button);
    }
  }

  async function toggleProvider(provider, button) {
    const nextEnabled = !provider.enabled;
    const action = nextEnabled ? "Enable" : "Disable";
    if (!window.confirm(`${action} provider “${provider.name}”?`)) {
      return;
    }
    const busyKey = `toggle-provider:${provider.name}`;
    if (!beginBusy(busyKey, button)) {
      return;
    }
    const session = captureSession();
    clearError();
    setNotice("");
    try {
      await apiRequest(
        `/admin/providers/${encodeURIComponent(provider.name)}`,
        { method: "PATCH", body: JSON.stringify({ enabled: nextEnabled }) },
        session,
      );
      ensureCurrent(session);
      await refreshAfterMutation(session, `Provider ${action.toLowerCase()}d.`);
    } catch (error) {
      if (error?.name !== "AbortError") {
        reportApplyFailure(error, `Provider ${action.toLowerCase()} failed.`);
      }
    } finally {
      endBusy(busyKey, button);
    }
  }

  async function deleteProvider(provider, button) {
    if (!window.confirm(`Delete provider “${provider.name}”?`)) {
      return;
    }
    const busyKey = `delete-provider:${provider.name}`;
    if (!beginBusy(busyKey, button)) {
      return;
    }
    const session = captureSession();
    clearError();
    setNotice("");
    try {
      await apiRequest(
        `/admin/providers/${encodeURIComponent(provider.name)}`,
        { method: "DELETE" },
        session,
      );
      ensureCurrent(session);
      if (editingName === provider.name) {
        clearEditor();
      }
      await refreshAfterMutation(session, "Provider deleted.");
    } catch (error) {
      if (error?.name !== "AbortError") {
        setNotice(error.message || "Provider deletion failed.");
        reportRequestError(error, "Provider deletion failed.");
      }
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
    setNotice("");
    setStatus("Refreshing providers…");
    try {
      await load();
      setStatus("Providers refreshed.");
    } catch (error) {
      reportRequestError(error, "Provider refresh failed.");
    } finally {
      button.disabled = false;
    }
  }

  byId("create-provider-form").addEventListener("submit", (event) =>
    void createProvider(event),
  );
  byId("edit-provider-form").addEventListener("submit", (event) =>
    void saveProvider(event),
  );
  byId("cancel-provider-edit").addEventListener("click", clearEditor);
  byId("refresh-providers").addEventListener("click", (event) =>
    void refresh(event.currentTarget),
  );

  return { load, loadOnce, reset };
};
