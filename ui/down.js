"use strict";

const panelId = new URLSearchParams(location.search).get("panel") || "";
let busy = false;
let lastSig = "";

function setMessage(text) {
  const m = document.getElementById("message");
  m.textContent = text || "";
  m.classList.toggle("hidden", !text);
}

function setError(e) {
  const box = document.getElementById("error");
  box.textContent = e ? errorText(e) : "";
  box.classList.toggle("hidden", !e);
}

function step(label, value, cls) {
  return h("div", { class: "step" },
    h("span", { class: `dot ${cls}` }),
    h("span", { class: "label" }, label),
    h("span", { class: `val status ${cls}` }, value));
}

async function act(cmd, args, doneText) {
  if (busy) return;
  busy = true;
  setError(null);
  setMessage(t("starting"));
  lastSig = "";
  await refresh();
  try {
    await invoke(cmd, args);
    setMessage(doneText);
  } catch (e) {
    setMessage("");
    setError(e);
  }
  busy = false;
  lastSig = "";
  await refresh();
}

function render(d) {
  const p = d.panel;
  document.title = p.name;
  document.getElementById("avatar").replaceChildren(avatar(p));
  document.getElementById("url").textContent = p.url;
  const head = p.state === "checking" ? t("connecting", p.name)
    : p.state === "error" ? t("serverError", p.name) : t("cantReach", p.name);
  document.getElementById("headline").textContent = head;
  document.getElementById("intro").textContent = p.state === "checking" ? "" : t("downIntro");

  const steps = [];
  if (d.local && d.engine) {
    const e = d.engine;
    steps.push(step(t("dockerEngine"),
      e === "running" ? t("engineRunning") : e === "not_running" ? t("engineStopped") : t("engineMissing"),
      e === "running" ? "up" : "down"));
  }
  if (d.containers.length) {
    for (const c of d.containers) {
      steps.push(step(t("container", c.name), c.running ? t("stateRunning") : c.state, c.running ? "up" : "down"));
    }
  }
  if (p.state !== "checking") {
    steps.push(step(t("panelAddress"), p.status ? `HTTP ${p.status}` : (p.detail || t("noAnswer")), p.state === "error" ? "error" : "down"));
  }
  document.getElementById("steps").replaceChildren(...steps);

  const actions = [];
  const disabled = busy || d.busy;
  if (d.local && d.engine === "not_running" && d.can_start_desktop) {
    actions.push(h("button", { id: "start-docker", class: "primary", disabled, onclick: () => act("start_docker", {}, t("dockerStarting")) }, t("startDocker")));
  }
  if (d.local && d.engine === "running" && d.containers.some((c) => !c.running)) {
    actions.push(h("button", { id: "start-containers", class: "primary", disabled, onclick: () => act("start_containers", { id: panelId }, t("containersStarted")) }, t("startContainers")));
  }
  actions.push(h("button", { id: "try-again", disabled, onclick: async () => { await invoke("check_now"); lastSig = ""; refresh(); } }, t("tryAgain")));
  actions.push(h("button", { id: "edit", class: "ghost", onclick: () => invoke("show_settings") }, t("editPanel")));
  document.getElementById("actions").replaceChildren(...actions);
  document.getElementById("retry").textContent = t("retrying");
}

async function refresh() {
  try {
    const d = await invoke("panel_detail", { id: panelId });
    const sig = JSON.stringify([d, busy]);
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
    await initLang();
  } catch (e) {
    setError(e);
    return;
  }
  await refresh();
  setInterval(refresh, 2000);
})();
