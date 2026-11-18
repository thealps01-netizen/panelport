"use strict";

const state = { panels: [], candidates: [], info: null, finding: false };

function renderNotice() {
  const box = document.getElementById("notice");
  const n = state.info && state.info.notice;
  if (!n) { box.classList.add("hidden"); return; }
  let text = "";
  if (n.kind === "upgraded") text = t("noticeUpgraded");
  else if (n.kind === "corrupt") text = t("noticeCorrupt") + n.moved_to;
  else if (n.kind === "too_new") text = t("noticeTooNew");
  const btn = h("button", { class: "ghost", onclick: async () => {
    try { await invoke("dismiss_notice"); } catch (e) { console.error(e); }
    state.info.notice = null; renderNotice();
  } }, t("dismiss"));
  box.replaceChildren(h("p", {}, text), btn);
  box.classList.remove("hidden");
}

function renderPanels() {
  const grid = document.getElementById("grid");
  const empty = document.getElementById("empty");
  const summary = document.getElementById("summary");
  const panels = state.panels;
  empty.classList.toggle("hidden", panels.length > 0);
  grid.classList.toggle("hidden", panels.length === 0);
  summary.classList.toggle("hidden", panels.length === 0);
  const up = panels.filter((p) => p.state === "up").length;
  const down = panels.filter((p) => p.state === "down").length;
  summary.replaceChildren(...[
    h("span", { class: "chip" }, t("nPanels", panels.length)),
    up ? h("span", { class: "chip" }, h("span", { class: "dot up" }), t("nUp", up)) : null,
    down ? h("span", { class: "chip" }, h("span", { class: "dot down" }), t("nDown", down)) : null,
  ].filter(Boolean));
  // Rebuild only when something visible changed, so hover/focus is not lost every poll.
  const sig = JSON.stringify(panels.map((p) => [p.id, p.name, p.url, p.state, !!p.icon_data]));
  if (grid.dataset.sig === sig) return;
  grid.dataset.sig = sig;
  grid.replaceChildren(...panels.map((p) =>
    h("button", { class: "card", "data-id": p.id, onclick: () => openPanel(p.id), title: p.url },
      h("div", { class: "head" }, avatar(p),
        h("div", { class: "minw0" },
          h("div", { class: "name" }, p.name),
          h("div", { class: "url muted small mono" }, p.url))),
      h("div", { class: "foot" }, statusEl(p.state), h("span", { class: "open-hint" }, t("open"), icon("arrow"))))
  ));
}

async function openPanel(id) {
  try { await invoke("open_panel", { id }); } catch (e) { alert(errorText(e)); }
}

async function refresh() {
  try {
    state.panels = await invoke("list_panels");
    renderPanels();
  } catch (e) {
    console.error("list_panels failed", e);
  }
}

// ---- Find panels -------------------------------------------------------------------------
function selectedCandidates() {
  return [...document.querySelectorAll("#find-body .cand")]
    .filter((row) => row.querySelector("input[type=checkbox]").checked)
    .map((row) => ({ name: row.querySelector("input[type=text]").value, url: row.dataset.url }));
}

function updateAddButton() {
  const n = selectedCandidates().length;
  const btn = document.getElementById("find-add");
  btn.textContent = t("addSelected", n);
  btn.disabled = n === 0;
}

function renderCandidates() {
  const body = document.getElementById("find-body");
  if (state.finding) {
    body.replaceChildren(h("div", { class: "loading" }, h("div", { class: "spinner" }), h("span", { class: "muted" }, t("searching"))));
    updateAddButton();
    return;
  }
  if (state.candidates.length === 0) {
    body.replaceChildren(h("p", { class: "muted" }, t("noneFound")));
    updateAddButton();
    return;
  }
  body.replaceChildren(...state.candidates.map((c) =>
    h("label", { class: "cand", "data-url": c.url },
      h("input", { type: "checkbox", checked: c.recommended, onchange: updateAddButton }),
      h("div", { class: "minw0" },
        h("input", { type: "text", value: c.name, "aria-label": t("name"), maxlength: "60", oninput: updateAddButton }),
        h("div", { class: "meta" },
          h("span", { class: "mono muted" }, c.url),
          c.container ? h("span", { class: "badge" }, t("fromDocker", c.container)) : h("span", { class: "badge port" }, t("port", c.port)),
          c.title && c.title !== c.name ? h("span", { class: "muted small" }, `“${c.title}”`) : null)),
      h("span")
    )));
  updateAddButton();
}

async function runDiscovery() {
  state.finding = true;
  document.getElementById("find-error").classList.add("hidden");
  renderCandidates();
  try {
    state.candidates = await invoke("discover");
  } catch (e) {
    state.candidates = [];
    showFindError(e);
  }
  state.finding = false;
  renderCandidates();
}

function showFindError(e) {
  const box = document.getElementById("find-error");
  box.textContent = errorText(e);
  box.classList.remove("hidden");
}

function openFind() {
  document.getElementById("find").classList.remove("hidden");
  runDiscovery();
}

async function closeFind() {
  document.getElementById("find").classList.add("hidden");
  try { await invoke("mark_discovery_offered"); } catch (e) { console.error(e); }
}

async function addSelected() {
  const items = selectedCandidates();
  if (!items.length) return;
  try {
    state.panels = await invoke("add_panels", { items });
    document.getElementById("find").classList.add("hidden");
    renderPanels();
  } catch (e) {
    showFindError(e);
  }
}

// Called by the native side (tray "Find panels…") when the launcher is already open.
window.panelport = { openFind };

(async function main() {
  document.getElementById("btn-settings").append(icon("gear"));
  document.getElementById("btn-find").prepend(icon("search"));
  document.getElementById("btn-find").onclick = openFind;
  document.getElementById("empty-find").onclick = openFind;
  document.getElementById("btn-settings").onclick = () => invoke("show_settings");
  document.getElementById("empty-add").onclick = () => invoke("show_settings");
  document.getElementById("find-cancel").onclick = closeFind;
  document.getElementById("find-again").onclick = runDiscovery;
  document.getElementById("find-add").onclick = addSelected;
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !document.getElementById("find").classList.contains("hidden")) closeFind();
  });
  try {
    state.info = await initLang();
  } catch (e) {
    console.error("app_info failed", e);
  }
  renderNotice();
  await refresh();
  if (location.hash === "#find" || (state.info && state.info.first_run)) openFind();
  setInterval(refresh, 2000);
})();
