"use strict";

window.createProviderHealthView = function createProviderHealthView({
  apiRequest,
  byId,
  captureSession,
  clearError,
  ensureCurrent,
  reportRequestError,
  setStatus,
  textCell,
}) {
  let snapshot = null;
  let httpStatus = null;
  let requestError = null;
  let requestVersion = 0;

  function formatDateTime(value) {
    if (!value) {
      return "Not probed";
    }
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? "Unknown" : date.toLocaleString();
  }

  function healthStateCell(status) {
    const cell = document.createElement("td");
    const badge = document.createElement("span");
    badge.className = "health-state";
    badge.dataset.healthState = status;
    badge.textContent = status;
    cell.append(badge);
    return cell;
  }

  function normalizedDetails() {
    const details = snapshot?.providers?.provider_details;
    if (!Array.isArray(details)) {
      return [];
    }
    return details.map((provider) => ({
      ...provider,
      status: ["healthy", "unhealthy", "unknown", "disabled"].includes(
        provider?.status,
      )
        ? provider.status
        : "unknown",
    }));
  }

  function render() {
    const providers = snapshot?.providers;
    const details = normalizedDetails();
    byId("health-body").replaceChildren(
      ...details.map((provider) => {
        const row = document.createElement("tr");
        const enabled = provider.status !== "disabled";
        const detail =
          provider.error_message ||
          (provider.response_time_ms == null
            ? ""
            : `${provider.response_time_ms} ms response`);
        row.append(
          textCell(provider.name),
          textCell(enabled ? "Enabled" : "Disabled"),
          healthStateCell(provider.status),
          textCell(formatDateTime(provider.last_check)),
          textCell(detail),
        );
        return row;
      }),
    );

    if (!snapshot) {
      byId("health-summary").textContent = "";
      byId("health-notice").textContent = requestError
        ? `Provider health unavailable: ${requestError}`
        : "";
      byId("health-empty").hidden = true;
      return;
    }

    const unknownCount = details.filter(
      (provider) => provider.status === "unknown",
    ).length;
    const unhealthyCount = details.filter(
      (provider) => provider.status === "unhealthy",
    ).length;
    byId("health-summary").textContent = [
      `${providers.enabled_providers ?? 0} enabled`,
      `${providers.healthy_providers ?? 0} healthy`,
      `${unknownCount} unknown`,
      `${unhealthyCount} unhealthy`,
      `${providers.total_providers ?? details.length} configured`,
      `Aggregate: ${providers.aggregate || "unknown"}`,
    ].join(" · ");

    const notices = [];
    if (httpStatus === 503) {
      notices.push("Gateway reported degraded health (HTTP 503).");
    }
    if (unknownCount > 0) {
      notices.push("Some enabled providers have unknown probe status.");
    }
    if (notices.length === 0 && snapshot.timestamp) {
      notices.push(`Snapshot from ${formatDateTime(snapshot.timestamp)}.`);
    }
    byId("health-notice").textContent = notices.join(" ");
    byId("health-empty").hidden = details.length !== 0;
  }

  function reset() {
    requestVersion += 1;
    snapshot = null;
    httpStatus = null;
    requestError = null;
    byId("refresh-health").disabled = false;
    render();
  }

  async function load(session = captureSession()) {
    const version = ++requestVersion;
    try {
      const result = await apiRequest("/health/detailed", {}, session, {
        acceptedStatuses: [503],
        includeStatus: true,
      });
      ensureCurrent(session);
      if (version !== requestVersion) {
        throw new DOMException("Stale provider health response", "AbortError");
      }
      if (!Array.isArray(result.data?.providers?.provider_details)) {
        throw new Error(
          "Provider health response did not include provider details.",
        );
      }
      snapshot = result.data;
      httpStatus = result.status;
      requestError = null;
      render();
    } catch (error) {
      if (error?.name !== "AbortError") {
        ensureCurrent(session);
        if (version === requestVersion) {
          snapshot = null;
          httpStatus = null;
          requestError = error.message || "Unknown request failure";
          render();
        }
      }
      throw error;
    }
  }

  async function refresh(button) {
    if (button.disabled) {
      return;
    }
    button.disabled = true;
    clearError();
    setStatus("Refreshing provider health…");
    try {
      await load();
      setStatus("Provider health refreshed.");
    } catch (error) {
      reportRequestError(error, "Provider health refresh failed.");
    } finally {
      button.disabled = false;
    }
  }

  byId("refresh-health").addEventListener("click", (event) =>
    void refresh(event.currentTarget),
  );

  return { load, reset };
};
