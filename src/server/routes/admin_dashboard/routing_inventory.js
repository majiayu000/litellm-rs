"use strict";

window.createRoutingInventoryView = function createRoutingInventoryView({
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
  let requestError = null;
  let requestVersion = 0;

  function healthStateCell(status) {
    const cell = document.createElement("td");
    const badge = document.createElement("span");
    badge.className = "health-state";
    const health = ["unknown", "healthy", "degraded", "unhealthy"].includes(
      status,
    )
      ? status
      : "unknown";
    badge.dataset.healthState = health;
    badge.textContent = health;
    cell.append(badge);
    return cell;
  }

  function formatList(values) {
    if (!Array.isArray(values) || values.length === 0) {
      return "None";
    }
    return values.join(", ");
  }

  function formatRate(window) {
    if (!window || typeof window !== "object") {
      return "—";
    }
    const usage =
      window.current_usage == null ? "—" : String(window.current_usage);
    const limit =
      window.configured_limit == null
        ? "unlimited"
        : String(window.configured_limit);
    return `${usage} / ${limit}`;
  }

  function formatCooldown(cooldown) {
    if (!cooldown || typeof cooldown !== "object") {
      return "None";
    }
    if (cooldown.remaining_secs == null) {
      return "Active";
    }
    return `${cooldown.remaining_secs}s remaining`;
  }

  function models() {
    return Array.isArray(snapshot?.models) ? snapshot.models : [];
  }

  function unavailableProviders() {
    return Array.isArray(snapshot?.unavailable_providers)
      ? snapshot.unavailable_providers
      : [];
  }

  function appendDeploymentRows(rows, model) {
    const deployments = Array.isArray(model.deployments)
      ? model.deployments
      : [];
    for (const deployment of deployments) {
      const row = document.createElement("tr");
      const reasons = Array.isArray(deployment.unavailable_reasons)
        ? deployment.unavailable_reasons
        : [];
      row.append(
        textCell(model.public_model),
        textCell(formatList(model.aliases)),
        textCell(deployment.provider),
        textCell(deployment.deployment),
        textCell(deployment.model),
        textCell(formatList(deployment.capabilities)),
        healthStateCell(deployment.health),
        textCell(deployment.available ? "Available" : "Unavailable"),
        textCell(formatList(reasons)),
        textCell(formatCooldown(deployment.cooldown)),
        textCell(formatRate(deployment.rpm)),
        textCell(formatRate(deployment.tpm)),
        textCell(
          deployment.active_requests == null
            ? "—"
            : String(deployment.active_requests),
        ),
      );
      rows.push(row);
    }
  }

  function render() {
    const deploymentRows = [];
    for (const model of models()) {
      appendDeploymentRows(deploymentRows, model);
    }
    byId("routing-body").replaceChildren(...deploymentRows);
    byId("routing-unavailable-body").replaceChildren(
      ...unavailableProviders().map((provider) => {
        const row = document.createElement("tr");
        const reasons = Array.isArray(provider.unavailable_reasons)
          ? provider.unavailable_reasons
          : [];
        row.append(
          textCell(provider.provider),
          textCell(provider.provider_type),
          textCell(formatList(provider.public_models)),
          textCell(provider.available ? "Available" : "Unavailable"),
          textCell(formatList(reasons)),
        );
        return row;
      }),
    );

    if (!snapshot) {
      byId("routing-summary").textContent = "";
      byId("routing-notice").textContent = requestError
        ? `Routing inventory unavailable: ${requestError}`
        : "";
      byId("routing-empty").hidden = true;
      byId("routing-unavailable-empty").hidden = true;
      return;
    }

    const deployments = models().flatMap((model) =>
      Array.isArray(model.deployments) ? model.deployments : [],
    );
    const gatedCount = unavailableProviders().filter((provider) =>
      (provider.unavailable_reasons || []).includes("feature_gated"),
    ).length;
    const unknownCount = deployments.filter(
      (deployment) => deployment.health === "unknown",
    ).length;
    const unhealthyCount = deployments.filter(
      (deployment) => deployment.health === "unhealthy",
    ).length;
    byId("routing-summary").textContent = [
      `Generation ${snapshot.snapshot_generation ?? "unknown"}`,
      `${models().length} public models`,
      `${deploymentRows.length} deployments`,
      `${unknownCount} unknown`,
      `${unhealthyCount} unhealthy`,
      `${unavailableProviders().length} unavailable providers`,
    ].join(" · ");

    const notices = [];
    if (gatedCount > 0) {
      notices.push(
        `${gatedCount} configured provider(s) are feature-gated and never became deployments.`,
      );
    }
    if (unknownCount > 0) {
      notices.push("Some deployments have unknown probe health, not healthy.");
    }
    byId("routing-notice").textContent = notices.join(" ");
    byId("routing-empty").hidden = deploymentRows.length !== 0;
    byId("routing-unavailable-empty").hidden =
      unavailableProviders().length !== 0;
  }

  function reset() {
    requestVersion += 1;
    snapshot = null;
    requestError = null;
    byId("refresh-routing").disabled = false;
    render();
  }

  async function load(session = captureSession()) {
    const version = ++requestVersion;
    try {
      const data = await apiRequest("/admin/routing/inventory", {}, session);
      ensureCurrent(session);
      if (version !== requestVersion) {
        throw new DOMException("Stale routing inventory response", "AbortError");
      }
      if (!Array.isArray(data?.models) || !Array.isArray(data?.unavailable_providers)) {
        throw new Error(
          "Routing inventory response did not include models and unavailable providers.",
        );
      }
      snapshot = data;
      requestError = null;
      render();
    } catch (error) {
      if (error?.name !== "AbortError") {
        ensureCurrent(session);
        if (version === requestVersion) {
          snapshot = null;
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
    setStatus("Refreshing routing inventory…");
    try {
      await load();
      setStatus("Routing inventory refreshed.");
    } catch (error) {
      reportRequestError(error, "Routing inventory refresh failed.");
    } finally {
      button.disabled = false;
    }
  }

  byId("refresh-routing").addEventListener("click", (event) =>
    void refresh(event.currentTarget),
  );

  return { load, reset };
};
