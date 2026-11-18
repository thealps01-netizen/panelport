"use strict";

// "Not reachable" page shown inside a panel window while its panel does not answer.
const panelId = new URLSearchParams(location.search).get("panel") || "";
let busy = false;
let lastSig = "";
let logsAt = 0;
let logs = null;

function setError(e) {
  const box = document.getElementById("error");
  box.textContent = e ? errorText(e) : "";
  box.classList.toggle("hidden", !e);
}

function step(kind, title, detail) {
  const ic = kind === "ok" ? "check" : kind === "bad" ? "cross" : "clock";
  return h("div", { class: `step ${kind}` }, icon(ic), h("div", {}, title, detail ? h("div", { class: "d" }, detail) : null));
}

async function act(cmd, args, doneText) {
  if (busy) return;
  busy = true;
  setError(null);
  lastSig = "";
  await refresh();
  try {
    await invoke(cmd, args);
    toast(doneText);
  } catch (e) {
    setError(e);
  }
  busy = false;
  lastSig = "";
  await refresh();
}

function owner(d) {
  return d.containers.find((c) => c.owner) || d.containers[0] || null;
}

async function loadLogs(d) {
  const o = owner(d);
  if (!o || Date.now() - logsAt < 10000) return;
  logsAt = Date.now();
  try {
    const r = await invoke("container_logs", { id: panelId });
    logs = r.lines;
  } catch (e) {
    logs = [errorText(e)];
  }
  lastSig = "";
}

function render(d) {
  const p = d.panel;
  document.title = p.name;
  const name = document.getElementById("name");
  name.textContent = p.name;
  name.lang = nameLang(p.name);

  const o = owner(d);
  const stopped = d.containers.filter((c) => !c.running);
  const engine = d.engine;
  const statusText = p.state === "checking" ? t("checkingNow")
    : p.state === "not_page" ? t("not_page")
    : p.state === "error" ? t("errorTitle")
    : p.state === "client_error" ? `${t("client_error")}${p.status ? ` · HTTP ${p.status}` : ""}`
    : p.since_ms ? t("downSince", since(p.since_ms, true)) : t("down");
  document.getElementById("status").replaceChildren(stateEl(p.state, statusText, true));

  let lead;
  if (p.state === "not_page") lead = t("leadNotPage");
  else if (p.state === "error") lead = t("leadError");
  else if (d.local && engine === "not_running") lead = t("leadDocker");
  else if (d.local && engine === "running" && o && !o.running) lead = t("leadStopped", o.name);
  else lead = t("leadPlain");
  document.getElementById("lead").textContent = p.state === "checking" ? "" : lead;

  const disabled = busy || d.busy;
  const actions = [];
  if (d.local && engine === "not_running" && d.can_start_desktop) {
    actions.push(h("button", { class: "btn red", id: "start-docker", disabled, onclick: () => act("start_docker", {}, t("dockerStarting")) }, t("startDocker")));
  }
  if (d.local && engine === "running" && stopped.length) {
    actions.push(h("button", { class: "btn red", id: "start-containers", disabled, onclick: () => act("start_containers", { id: panelId }, t("containersStarted")) }, busy ? t("starting") : t("startContainers")));
  }
  actions.push(h("button", { class: "btn", id: "try-again", disabled, onclick: async () => { await invoke("check_now"); lastSig = ""; refresh(); } }, t("tryAgain")));
  actions.push(h("button", { class: "btn", id: "edit", onclick: () => invoke("show_view", { view: "settings" }) }, t("editPanel")));
  document.getElementById("actions").replaceChildren(...actions);

  const steps = [];
  if (d.local && engine) {
    if (engine === "running") steps.push(step("ok", t("engineOk"), t("engineOkD", d.engine_version)));
    else if (engine === "not_running") steps.push(step("bad", t("engineOff"), t("engineOffD")));
    else steps.push(step("warn", t("engineMissing"), t("engineMissingD")));
  }
  for (const c of d.containers) {
    if (c.running) steps.push(step("ok", t("contRunning", c.name), null));
    else steps.push(step("bad", t("contStopped", c.name), t("exitInfo", c.exit_code, c.finished_at ? clock(Date.parse(c.finished_at)) : "")));
  }
  if (p.state !== "checking") {
    const kind = p.state === "down" ? "warn" : "warn";
    const title = p.state === "not_page" ? t("addrNotPage") : p.state === "error" ? t("addrError") : t("addrNo");
    steps.push(step(kind, title, t("addrD", hostPort(p.url), detailText(p.detail))));
  }
  document.getElementById("steps").replaceChildren(...steps);
  document.getElementById("retry").textContent = t("retrying");

  const side = [h("h2", { class: "lab" }, t("details"))];
  const kv = h("div", { class: "kv" }, h("span", {}, t("kAddress")), h("span", { class: "mono" }, hostPort(p.url)));
  if (o) kv.append(h("span", {}, t("kContainer")), h("span", { class: "mono" }, o.name));
  if (d.project) kv.append(h("span", {}, t("kCompose")), h("span", { class: "mono" }, d.project));
  if (d.last_ok) {
    const [at, status, ms] = d.last_ok;
    kv.append(h("span", {}, t("kLastOk")), h("span", {}, `${clock(at)} · HTTP ${status} · ${ms} ms`));
  }
  side.push(kv);
  if (o) {
    side.push(h("h2", { class: "lab" }, t("lastLog")));
    side.push(h("div", { class: "log scroll" }, logs ? (logs.length ? logs.slice(-12).join("\n") : t("logsEmpty")) : "…"));
  }
  document.getElementById("side").replaceChildren(...side);
}

async function refresh() {
  try {
    const d = await invoke("panel_detail", { id: panelId });
    if (owner(d)) loadLogs(d).then(() => { if (!lastSig) refresh(); });
    const sig = JSON.stringify([d, busy, logs, Math.floor(Date.now() / 30000)]);
    if (sig !== lastSig) {
      lastSig = sig;
      render(d);
    }
  } catch (e) {
    setError(e);
  }
}

(async function main() {
  try {
    const info = await invoke("app_info");
    LANG = info.lang === "tr" ? "tr" : "en";
  } catch (e) {
    setError(e);
    return;
  }
  document.documentElement.lang = LANG;
  await refresh();
  setInterval(refresh, 2000);
})();
