"use strict";

const L = {
  info: null,
  view: "panels",
  panels: [],
  selected: null,
  docker: null,
  previews: {},          // id -> { stamp, url }
  logsFor: null,         // id whose log drawer is open
  find: { state: "idle", candidates: [], skipped: [], at: 0, names: {}, ticked: {} },
};

/* ---- Views ------------------------------------------------------------------------------- */

function show(view) {
  if (!["panels", "find", "settings"].includes(view)) view = "panels";
  L.view = view;
  for (const v of ["panels", "find", "settings"]) {
    document.getElementById(`view-${v}`).classList.toggle("hidden", v !== view);
    const tab = document.getElementById(`tab-${v}`);
    tab.setAttribute("aria-selected", String(v === view));
    tab.tabIndex = v === view ? 0 : -1;
  }
  if (location.hash !== `#${view}`) history.replaceState(null, "", `#${view}`);
  if (view === "panels") renderPanels();
  if (view === "find" && (L.find.state === "idle" || Date.now() - L.find.at > 60000) && L.find.state !== "scanning") scan();
  if (view === "settings" && window.settingsView) window.settingsView.render();
}

// The native side (tray menu, title bar of a panel window) calls this.
window.panelport = { show };

/* ---- Title bar --------------------------------------------------------------------------- */

function renderDock() {
  const el = document.getElementById("dock");
  const e = L.docker && L.docker.engine;
  el.className = "dockline" + (e === "running" ? " s-up" : e === "not_running" ? " s-down" : "");
  const text = e === "running" ? t("dockerRunning") : e === "not_running" ? t("dockerStopped") : e === "not_installed" ? t("dockerMissing") : t("dockerUnknown");
  el.lastElementChild.textContent = text;
  el.title = text;
}

/* ---- Panels view ------------------------------------------------------------------------- */

function selectedPanel() {
  return L.panels.find((p) => p.id === L.selected) || null;
}

function stateWord(p) {
  if (p.state === "down" && p.since_ms) return t("downFor", since(p.since_ms));
  return t(p.state);
}

function renderList() {
  const up = L.panels.filter((p) => p.state === "up").length;
  document.getElementById("list-head").textContent = t("listHead", L.panels.length, up);
  const list = document.getElementById("list");
  const sig = JSON.stringify([L.selected, L.panels.map((p) => [p.id, p.name, p.state, p.can_start, Math.floor((Date.now() - (p.since_ms || 0)) / 60000)])]);
  if (list.dataset.sig === sig) return;
  list.dataset.sig = sig;
  const hadFocus = list.contains(document.activeElement);
  list.replaceChildren(...L.panels.map((p) => {
    const item = h("div", {
      class: `item${p.state === "down" ? " dim" : ""}`, role: "option", id: `opt-${p.id}`,
      "aria-selected": String(p.id === L.selected),
      onclick: () => select(p.id),
      ondblclick: () => openPanel(p.id),
    }, nameEl("span", "n", p.name), stateEl(p.state, stateWord(p)));
    if (p.can_start) {
      item.append(h("button", { class: "btn solid small act", onclick: (e) => { e.stopPropagation(); startContainers(p.id, e.currentTarget); } }, t("startContainers")));
    }
    return item;
  }));
  list.setAttribute("aria-activedescendant", L.selected ? `opt-${L.selected}` : "");
  if (hadFocus) list.focus();
}

function select(id) {
  if (L.selected === id) return;
  L.selected = id;
  L.logsFor = null;
  renderPanels();
  const el = document.getElementById(`opt-${id}`);
  if (el) el.scrollIntoView({ block: "nearest" });
}

async function previewUrl(p) {
  if (!p.preview) return null;
  const c = L.previews[p.id];
  if (c && c.stamp === p.preview) return c.url;
  try {
    const url = await invoke("panel_preview", { id: p.id });
    L.previews[p.id] = { stamp: p.preview, url };
    return url;
  } catch (e) {
    console.error("panel_preview", e);
    return null;
  }
}

/** Shrinks the stage name until it fits two lines at most. */
function fitName(el) {
  const max = Math.min(128, Math.max(56, window.innerWidth * 0.086));
  let size = max;
  el.style.fontSize = `${size}px`;
  const lh = () => parseFloat(getComputedStyle(el).lineHeight) || size * 0.86;
  while (size > 40 && (el.scrollWidth > el.clientWidth + 1 || el.offsetHeight > lh() * 2.1)) {
    size -= 6;
    el.style.fontSize = `${size}px`;
  }
}

function avatarEl(p) {
  const av = h("div", { class: "mono-avatar", "aria-hidden": "true" });
  if (p.icon_data) av.append(h("img", { src: p.icon_data, alt: "" }));
  else av.append((p.name.trim()[0] || "?").toLocaleUpperCase(nameLang(p.name)));
  return av;
}

async function renderStage() {
  const stage = document.getElementById("stage");
  const p = selectedPanel();
  if (!p) {
    stage.classList.add("hidden");
    return;
  }
  stage.classList.remove("hidden");
  const url = await previewUrl(p);
  const sig = JSON.stringify([p.id, p.name, p.url, p.state, p.latency_ms, p.container, url ? p.preview : 0, L.logsFor === p.id, Math.floor((Date.now() - (p.since_ms || 0)) / 60000)]);
  if (stage.dataset.sig === sig) return;
  const sameShot = stage.dataset.shot === `${p.id}:${p.preview}` && !!url;
  stage.dataset.sig = sig;
  stage.setAttribute("aria-label", `${t("open")}: ${p.name}`);

  const keep = sameShot ? stage.querySelector("img.shot") : null;
  const parts = [];
  if (keep) parts.push(keep);
  else if (url) parts.push(h("img", { class: "shot", src: url, alt: "" }));
  else {
    const prefsOff = L.info && L.info.prefs && L.info.prefs.keep_previews === false;
    parts.push(h("div", { class: "blank" }, avatarEl(p), h("span", {}, prefsOff ? t("previewsOff") : t("noPreview"))));
  }
  stage.dataset.shot = url ? `${p.id}:${p.preview}` : "";

  const meta = h("div", { class: "meta" },
    h("span", { class: "mono" }, hostPort(p.url)),
    stateEl(p.state, stateWord(p), true),
    p.latency_ms != null && p.state !== "down" ? h("span", { class: "mono" }, `${p.latency_ms} ms`) : null,
    p.container ? h("span", {}, t("container", p.container)) : null);
  const big = nameEl("div", "big", p.name);
  parts.push(h("div", { class: "cap" }, big, meta));
  if (L.logsFor === p.id) parts.push(logsDrawer(p));
  stage.replaceChildren(...parts);
  requestAnimationFrame(() => fitName(big));
}

function renderRail() {
  const rail = document.getElementById("rail");
  const p = selectedPanel();
  if (!p) { rail.replaceChildren(); return; }
  const sig = JSON.stringify([p.id, !!p.container]);
  if (rail.dataset.sig === sig) return;
  rail.dataset.sig = sig;
  const ring = (cls, ic, label, onclick, extra) =>
    h("button", Object.assign({ class: `ring${cls ? " " + cls : ""}`, onclick }, extra || {}), h("span", {}, icon(ic)), label);
  rail.replaceChildren(
    ring("primary", "open", t("open"), () => openPanel(p.id)),
    ring("", "reload", t("reload"), () => { invoke("check_now"); L.previews[p.id] = null; refresh(); toast(t("checking")); }),
    p.container ? ring("", "logs", t("logs"), () => toggleLogs(p.id)) : null,
    ring("", "edit", t("edit"), () => { show("settings"); window.settingsView && window.settingsView.edit(p.id); }),
  );
}

function renderEmpty() {
  const stage = document.getElementById("stage");
  stage.classList.remove("hidden");
  stage.dataset.sig = "empty";
  stage.classList.add("empty-stage");
  stage.replaceChildren(h("div", { class: "empty" },
    h("h1", { class: "display" }, t("emptyTitle")),
    h("p", {}, t("emptyText")),
    h("div", { class: "row" },
      h("button", { class: "btn solid", onclick: (e) => { e.stopPropagation(); show("find"); } }, t("findNow")),
      h("button", { class: "btn", onclick: (e) => { e.stopPropagation(); show("settings"); window.settingsView && window.settingsView.addNew(); } }, t("addByAddress")))));
  document.getElementById("rail").replaceChildren();
  document.getElementById("rail").dataset.sig = "";
}

function renderPanels() {
  renderNotice();
  if (!L.panels.length) {
    L.selected = null;
    renderList();
    renderEmpty();
    return;
  }
  document.getElementById("stage").classList.remove("empty-stage");
  if (!selectedPanel()) L.selected = L.panels[0].id;
  renderList();
  renderStage();
  renderRail();
}

function renderNotice() {
  const n = L.info && L.info.notice;
  const list = document.querySelector(".list");
  let box = document.getElementById("notice");
  if (!n) { if (box) box.remove(); return; }
  if (box) return;
  const text = n.kind === "upgraded" ? t("noticeUpgraded") : n.kind === "corrupt" ? t("noticeCorrupt") + n.moved_to : t("noticeTooNew");
  box = h("div", { class: "banner", id: "notice", role: "status" }, h("p", {}, text),
    h("button", { class: "btn small", onclick: async () => { try { await invoke("dismiss_notice"); } catch (e) { console.error(e); } L.info.notice = null; renderNotice(); } }, t("dismiss")));
  list.prepend(box);
}

/* ---- Actions ----------------------------------------------------------------------------- */

async function openPanel(id) {
  try { await invoke("open_panel", { id }); } catch (e) { toast(errorText(e), true); }
}

async function startContainers(id, btn) {
  if (btn) { btn.disabled = true; btn.textContent = t("starting"); }
  try {
    await invoke("start_containers", { id });
    toast(t("containersStarted"));
  } catch (e) {
    toast(errorText(e), true);
  }
  refresh();
}

function logsDrawer(p) {
  const pre = h("pre", { class: "scroll", id: "logs-pre" }, "…");
  const box = h("section", { class: "logs", onclick: (e) => e.stopPropagation(), "aria-label": t("logs") },
    h("header", {}, h("span", { class: "lab", id: "logs-title" }, t("logsOf", p.container || "")), h("span", { class: "grow" }),
      h("button", { class: "btn small", onclick: () => loadLogs(p.id) }, t("refreshLogs")),
      h("button", { class: "btn small", onclick: () => toggleLogs(p.id) }, t("closeLogs"))),
    pre);
  loadLogs(p.id);
  return box;
}

async function loadLogs(id) {
  try {
    const r = await invoke("container_logs", { id });
    const pre = document.getElementById("logs-pre");
    if (!pre) return;
    document.getElementById("logs-title").textContent = t("logsOf", r.container);
    pre.textContent = r.lines.length ? r.lines.join("\n") : t("logsEmpty");
    pre.scrollTop = pre.scrollHeight;
  } catch (e) {
    const pre = document.getElementById("logs-pre");
    if (pre) pre.textContent = errorText(e);
  }
}

function toggleLogs(id) {
  L.logsFor = L.logsFor === id ? null : id;
  renderStage();
}

/* ---- Find panels ------------------------------------------------------------------------- */

function ago(ms) {
  const s = Math.round((Date.now() - ms) / 1000);
  if (s < 5) return t("justNow");
  if (s < 60) return t("secondsAgo", s);
  return t("minutesAgo", Math.round(s / 60));
}

async function scan() {
  L.find.state = "scanning";
  renderFind();
  try {
    const r = await invoke("discover");
    L.find.candidates = r.candidates;
    L.find.skipped = r.skipped;
    L.find.at = Date.now();
    L.find.state = "done";
    L.find.ticked = {};
    L.find.names = {};
    for (const c of r.candidates) if (c.recommended && !c.known) L.find.ticked[c.url] = true;
  } catch (e) {
    L.find.state = "error";
    L.find.error = errorText(e);
  }
  renderFind();
}

function tickedList() {
  return L.find.candidates.filter((c) => !c.known && L.find.ticked[c.url]);
}

function renderFindCard() {
  const card = document.getElementById("find-card");
  const sel = tickedList();
  const names = sel.map((c) => L.find.names[c.url] || c.name);
  const sum = h("div", { class: "sum" }, t("willAdd", sel.length), " ",
    ...names.flatMap((n, i) => [i ? ", " : "", h("b", { lang: nameLang(n) }, n)]));
  card.replaceChildren(
    h("div", { class: "lab" }, t("selected")),
    sum,
    h("button", { class: "btn solid", id: "find-add", disabled: sel.length === 0 || L.find.state === "scanning", onclick: addSelected }, t("addN", sel.length)),
    h("button", { class: "btn", id: "find-again", disabled: L.find.state === "scanning", onclick: scan }, t("rescan")),
    h("div", { class: "note" }, t("scanNote")));
}

function renderFind() {
  const title = document.getElementById("find-title");
  const meta = document.getElementById("find-meta");
  const rows = document.getElementById("find-rows");
  if (L.find.state === "scanning") {
    title.textContent = t("find");
    meta.textContent = "";
    rows.replaceChildren(h("div", { class: "scanning", role: "status" }, h("div", { class: "sweep" }), t("scanning")));
    renderFindCard();
    return;
  }
  if (L.find.state === "error") {
    title.textContent = t("find");
    meta.textContent = "";
    rows.replaceChildren(h("div", { class: "errline", role: "alert" }, L.find.error));
    renderFindCard();
    return;
  }
  const fresh = L.find.candidates.filter((c) => !c.known);
  title.textContent = fresh.length ? t("foundN", fresh.length) : t("foundNone");
  meta.textContent = L.find.at ? t("scannedAgo", ago(L.find.at)) : "";
  const list = L.find.candidates.map((c) => {
    const ticked = !c.known && !!L.find.ticked[c.url];
    const name = L.find.names[c.url] || c.name;
    const src = [c.title ? t("srcTitle", c.title) : null, c.container ? t("srcDocker", c.container) : t("srcLocal")].filter(Boolean).join(" · ");
    const box = h("input", { type: "checkbox", checked: ticked, disabled: c.known, "aria-label": name,
      onchange: (e) => { L.find.ticked[c.url] = e.target.checked; renderFind(); } });
    const input = h("input", { class: "name", type: "text", value: name, maxlength: "60", autocomplete: "off", spellcheck: "false",
      disabled: c.known, lang: nameLang(name), "aria-label": t("name"),
      oninput: (e) => { L.find.names[c.url] = e.target.value; e.target.lang = nameLang(e.target.value); renderFindCard(); } });
    return h("div", { class: `frow${ticked ? " sel" : ""}`, "data-url": c.url },
      h("label", { class: "check" }, box),
      h("div", { class: "minw" }, input, h("div", { class: "src", title: src }, src)),
      h("span", { class: "mono" }, hostPort(c.url)),
      h("span", { class: "tag" }, c.known ? t("tagKnown") : c.container ? t("tagDocker") : t("tagLocal")));
  });
  const keepFocus = document.activeElement && document.activeElement.closest(".frow");
  const focusUrl = keepFocus ? keepFocus.dataset.url : null;
  const focusName = document.activeElement && document.activeElement.classList.contains("name");
  rows.replaceChildren(...list);
  if (L.find.skipped.length) {
    const items = L.find.skipped.map((s) => `localhost:${s.port} · ${s.reason}`).join(", ");
    rows.append(h("div", { class: "skip" }, t("skipped", L.find.skipped.length, items)));
  }
  if (focusUrl) {
    const row = rows.querySelector(`.frow[data-url="${CSS.escape(focusUrl)}"]`);
    const el = row && row.querySelector(focusName ? "input.name" : "input[type=checkbox]");
    if (el) el.focus();
  }
  renderFindCard();
}

async function addSelected() {
  const items = tickedList().map((c) => ({ name: (L.find.names[c.url] || c.name).trim(), url: c.url }));
  if (!items.length) return;
  try {
    L.panels = await invoke("add_panels", { items });
    toast(t("added", items.length));
    for (const c of L.find.candidates) if (L.find.ticked[c.url]) { c.known = true; L.find.ticked[c.url] = false; }
    const last = L.panels[L.panels.length - 1];
    if (last) L.selected = last.id;
    renderFind();
    show("panels");
  } catch (e) {
    toast(errorText(e), true);
  }
}

/* ---- Polling and keyboard ---------------------------------------------------------------- */

async function refresh() {
  try {
    const [panels, docker] = await Promise.all([invoke("list_panels"), invoke("docker_status")]);
    L.panels = panels;
    L.docker = docker;
    renderDock();
    if (L.view === "panels") renderPanels();
  } catch (e) {
    console.error("refresh", e);
  }
}

function onListKey(e) {
  if (!L.panels.length) return;
  const i = Math.max(0, L.panels.findIndex((p) => p.id === L.selected));
  let j = i;
  if (e.key === "ArrowDown") j = Math.min(L.panels.length - 1, i + 1);
  else if (e.key === "ArrowUp") j = Math.max(0, i - 1);
  else if (e.key === "Home") j = 0;
  else if (e.key === "End") j = L.panels.length - 1;
  else if (e.key === "Enter") { openPanel(L.panels[i].id); e.preventDefault(); return; }
  else return;
  e.preventDefault();
  select(L.panels[j].id);
}

function onTabsKey(e) {
  const order = ["panels", "find", "settings"];
  const i = order.indexOf(L.view);
  let j = null;
  if (e.key === "ArrowRight") j = (i + 1) % 3;
  else if (e.key === "ArrowLeft") j = (i + 2) % 3;
  if (j === null) return;
  e.preventDefault();
  show(order[j]);
  document.getElementById(`tab-${order[j]}`).focus();
}

(async function main() {
  try {
    L.info = await invoke("app_info");
    LANG = L.info.lang === "tr" ? "tr" : "en";
  } catch (e) {
    console.error("app_info", e);
  }
  document.documentElement.lang = LANG;
  applyStrings();
  wireTitlebar(document.getElementById("bar"));
  for (const b of document.querySelectorAll("#tabs button")) b.onclick = () => show(b.dataset.view);
  document.getElementById("tabs").addEventListener("keydown", onTabsKey);
  document.getElementById("add-panel").onclick = () => { show("settings"); window.settingsView && window.settingsView.addNew(); };
  const list = document.getElementById("list");
  list.addEventListener("keydown", onListKey);
  const stage = document.getElementById("stage");
  stage.addEventListener("click", () => { const p = selectedPanel(); if (p) openPanel(p.id); });
  stage.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") { const p = selectedPanel(); if (p) { e.preventDefault(); openPanel(p.id); } }
    if (e.key === "Escape" && L.logsFor) toggleLogs(L.logsFor);
  });
  window.addEventListener("resize", () => { const b = document.querySelector(".stage .big"); if (b) fitName(b); });
  await refresh();
  const wanted = (location.hash || "#panels").slice(1);
  show(L.info && L.info.first_run && wanted === "panels" ? "find" : wanted);
  if (L.info && L.info.first_run) invoke("mark_discovery_offered").catch(() => {});
  setInterval(refresh, 2000);
  setInterval(() => { if (L.view === "find" && L.find.state === "done") document.getElementById("find-meta").textContent = t("scannedAgo", ago(L.find.at)); }, 5000);
})();
