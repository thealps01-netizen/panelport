"use strict";

let rows = [];      // [{id, name, url, health_path, icon, icon_data}]
let saved = "";     // JSON of the last saved list, to detect unsaved changes
let info = null;

function snapshot() {
  return JSON.stringify(rows.map((r) => [r.id, r.name, r.url, r.health_path, r.icon]));
}

function markDirty() {
  const dirty = snapshot() !== saved;
  document.getElementById("save-status").textContent = dirty ? t("unsaved") : "";
  document.getElementById("save").disabled = !dirty || (info && info.read_only);
}

function showError(e) {
  const box = document.getElementById("error");
  if (!e) { box.classList.add("hidden"); return; }
  box.textContent = errorText(e);
  box.classList.remove("hidden");
}

function field(labelKey, input) {
  return h("label", { class: "field" }, h("span", {}, t(labelKey)), input);
}

function renderRows() {
  const box = document.getElementById("rows");
  if (rows.length === 0) {
    box.replaceChildren(h("p", { class: "muted" }, t("emptyText")));
    markDirty();
    return;
  }
  box.replaceChildren(...rows.map((r, i) => {
    const bind = (key) => (e) => { r[key] = e.target.value; markDirty(); };
    const iconSel = h("select", { onchange: bind("icon") },
      h("option", { value: "favicon", selected: r.icon === "favicon" }, t("iconFavicon")),
      h("option", { value: "monogram", selected: r.icon === "monogram" }, t("iconMonogram")));
    const move = (d) => () => {
      const j = i + d;
      if (j < 0 || j >= rows.length) return;
      [rows[i], rows[j]] = [rows[j], rows[i]];
      renderRows();
    };
    return h("div", { class: "prow" },
      avatar({ ...r, icon_data: r.icon === "favicon" ? r.icon_data : null, name: r.name || "?" }),
      field("name", h("input", { type: "text", value: r.name, maxlength: "60", autocomplete: "off", oninput: bind("name") })),
      field("address", h("input", { type: "text", value: r.url, placeholder: "http://localhost:3552", spellcheck: "false", autocomplete: "off", oninput: bind("url") })),
      field("healthPath", h("input", { type: "text", value: r.health_path, placeholder: "/", spellcheck: "false", autocomplete: "off", oninput: bind("health_path") })),
      field("icon", iconSel),
      h("div", { class: "ops" },
        h("button", { class: "ghost icon", title: t("moveUp"), "aria-label": t("moveUp"), disabled: i === 0, onclick: move(-1) }, icon("up")),
        h("button", { class: "ghost icon", title: t("moveDown"), "aria-label": t("moveDown"), disabled: i === rows.length - 1, onclick: move(1) }, icon("down")),
        h("button", { class: "ghost icon danger", title: t("remove"), "aria-label": t("remove"), onclick: () => { rows.splice(i, 1); renderRows(); } }, icon("trash"))));
  }));
  markDirty();
}

async function load() {
  const list = await invoke("list_panels");
  rows = list.map((p) => ({ id: p.id, name: p.name, url: p.url, health_path: p.health_path, icon: p.icon, icon_data: p.icon_data }));
  saved = snapshot();
  renderRows();
}

async function save() {
  showError(null);
  const btn = document.getElementById("save");
  btn.disabled = true;
  try {
    const panels = rows.map((r) => ({ id: r.id || "", name: r.name, url: r.url, health_path: r.health_path, icon: r.icon }));
    const list = await invoke("save_panels", { panels });
    rows = list.map((p) => ({ id: p.id, name: p.name, url: p.url, health_path: p.health_path, icon: p.icon, icon_data: p.icon_data }));
    saved = snapshot();
    renderRows();
    document.getElementById("save-status").textContent = t("saved");
  } catch (e) {
    showError(e);
    markDirty();
  }
}

function selectTab(name) {
  for (const n of ["panels", "general", "about"]) {
    document.getElementById(`tab-${n}`).setAttribute("aria-selected", String(n === name));
    document.getElementById(`sec-${n}`).classList.toggle("hidden", n !== name);
  }
  document.getElementById("savebar").classList.toggle("hidden", name !== "panels");
}

(async function main() {
  try {
    info = await initLang();
  } catch (e) {
    showError(e);
    return;
  }
  document.getElementById("add-row").prepend(icon("plus"));
  document.getElementById("find").prepend(icon("search"));
  document.getElementById("version").textContent = t("version", info.version);
  document.getElementById("log-dir").textContent = info.log_dir;
  document.getElementById("data-dir").textContent = info.data_dir;
  for (const n of ["panels", "general", "about"]) document.getElementById(`tab-${n}`).onclick = () => selectTab(n);
  document.getElementById("add-row").onclick = () => {
    rows.push({ id: "", name: t("newPanel"), url: "http://localhost:", health_path: "/", icon: "favicon", icon_data: null });
    renderRows();
    const inputs = document.querySelectorAll(".prow input[type=text]");
    if (inputs.length >= 2) inputs[inputs.length - 2].focus();
  };
  document.getElementById("find").onclick = () => invoke("show_launcher", { find: true });
  document.getElementById("save").onclick = save;
  document.getElementById("open-logs").onclick = async () => {
    try { await invoke("open_log_folder"); } catch (e) { selectTab("general"); showError(e); }
  };
  document.getElementById("releases").onclick = () => invoke("open_releases");
  const auto = document.getElementById("autostart");
  auto.checked = await invoke("get_autostart");
  auto.onchange = async () => { auto.checked = await invoke("set_autostart", { enabled: auto.checked }); };
  await load();
  // Pick up panels added from the launcher while this window is open (only when nothing is being edited).
  setInterval(async () => {
    if (snapshot() !== saved) return;
    const list = await invoke("list_panels");
    const fresh = JSON.stringify(list.map((p) => [p.id, p.name, p.url, p.health_path, p.icon]));
    if (fresh !== saved) await load();
  }, 3000);
})();
