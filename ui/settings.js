"use strict";

// Settings view of the launcher: sections Panels, General, Notifications, Saved passwords, About.
(function () {
  const S = { panels: [], editing: null, adding: false, removing: null, error: null, prefs: null, autostart: false, rendered: false };
  const SECTIONS = [
    ["sec-panels", "panels"],
    ["sec-general", "general"],
    ["sec-notifications", "notifications"],
    ["sec-passwords", "savedPasswords"],
    ["sec-about", "about"],
  ];
  const isWindows = () => L.info && L.info.os === "windows";

  function toggle(id, checked, onchange, label) {
    return h("label", { class: "toggle" }, h("input", { type: "checkbox", id, checked, onchange, "aria-label": label }), h("span"));
  }

  function itemRow(title, desc, side, extra) {
    return h("div", { class: "item-row" }, h("div", {}, h("div", { class: "t" }, title), desc ? h("div", { class: "d" }, desc) : null, extra || null), h("div", { class: "sidebtns" }, side));
  }

  /* ---- Panels ---------------------------------------------------------------------------- */

  function toInput(p) {
    return { id: p.id, name: p.name, url: p.url, health_path: p.health_path, icon: p.icon };
  }

  async function savePanels(list, quiet) {
    try {
      const fresh = await invoke("save_panels", { panels: list });
      S.panels = fresh;
      L.panels = fresh;
      S.error = null;
      if (!quiet) toast(t("saved"));
      return true;
    } catch (e) {
      S.error = errorText(e);
      return false;
    }
  }

  function editor(p, isNew) {
    const f = { name: p ? p.name : t("newPanel"), url: p ? p.url : "http://localhost:", health_path: p ? p.health_path : "/", icon: p ? p.icon : "favicon" };
    const name = h("input", { class: "input", type: "text", value: f.name, maxlength: "60", autocomplete: "off", spellcheck: "false", id: "ed-name" });
    const url = h("input", { class: "input mono", type: "text", value: f.url, autocomplete: "off", spellcheck: "false", placeholder: "http://localhost:3552", id: "ed-url" });
    const hp = h("input", { class: "input mono", type: "text", value: f.health_path, autocomplete: "off", spellcheck: "false", placeholder: "/", id: "ed-health" });
    const ic = h("select", { class: "input", id: "ed-icon" },
      h("option", { value: "favicon", selected: f.icon === "favicon" }, t("iconFavicon")),
      h("option", { value: "monogram", selected: f.icon === "monogram" }, t("iconMonogram")));
    const save = async () => {
      const entry = { id: p ? p.id : "", name: name.value, url: url.value, health_path: hp.value, icon: ic.value };
      const list = S.panels.map(toInput);
      if (p) list[list.findIndex((x) => x.id === p.id)] = entry; else list.push(entry);
      if (await savePanels(list)) { S.editing = null; S.adding = false; }
      render();
    };
    const cancel = () => { S.editing = null; S.adding = false; S.error = null; render(); };
    const form = h("div", { class: "editor", onkeydown: (e) => { if (e.key === "Enter" && e.target.tagName === "INPUT") save(); if (e.key === "Escape") cancel(); } },
      h("label", { class: "field" }, t("name"), name),
      h("label", { class: "field" }, t("address"), url),
      h("label", { class: "field" }, t("health"), hp),
      h("label", { class: "field" }, t("icon"), ic),
      h("div", { class: "actions" },
        h("button", { class: "btn solid", onclick: save }, isNew ? t("addPanel") : t("save")),
        h("button", { class: "btn", onclick: cancel }, t("cancel")),
        S.error ? h("div", { class: "errline", role: "alert" }, S.error) : null));
    setTimeout(() => { const el = document.getElementById("ed-name"); if (el) { el.focus(); el.select(); } }, 0);
    return form;
  }

  function move(id, delta) {
    const list = S.panels.map(toInput);
    const i = list.findIndex((x) => x.id === id);
    const j = i + delta;
    if (i < 0 || j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    savePanels(list, true).then(() => { render(); const g = document.querySelector(`.pl[data-id="${id}"] .grip`); if (g) g.focus(); });
  }

  // Pointer-based drag reorder (works the same with mouse, pen and touch).
  function startDrag(e, id) {
    if (e.button !== 0) return;
    e.preventDefault();
    const rows = [...document.querySelectorAll(".pl[data-id]")];
    const src = rows.find((r) => r.dataset.id === id);
    src.classList.add("dragging");
    let target = null, after = false;
    const onMove = (ev) => {
      rows.forEach((r) => r.classList.remove("drop-before", "drop-after"));
      target = null;
      for (const r of rows) {
        const b = r.getBoundingClientRect();
        if (ev.clientY >= b.top && ev.clientY <= b.bottom && r !== src) {
          after = ev.clientY > b.top + b.height / 2;
          target = r;
          r.classList.add(after ? "drop-after" : "drop-before");
        }
      }
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      src.classList.remove("dragging");
      rows.forEach((r) => r.classList.remove("drop-before", "drop-after"));
      if (!target) return;
      const list = S.panels.map(toInput);
      const from = list.findIndex((x) => x.id === id);
      const [item] = list.splice(from, 1);
      let to = list.findIndex((x) => x.id === target.dataset.id);
      if (after) to += 1;
      list.splice(to, 0, item);
      savePanels(list, true).then(render);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  function panelRow(p) {
    const grip = h("button", { class: "grip", "aria-label": `${p.name}: ${t("dragHint")}`, title: t("dragHint"),
      onpointerdown: (e) => startDrag(e, p.id),
      onkeydown: (e) => { if (e.altKey && e.key === "ArrowUp") { e.preventDefault(); move(p.id, -1); } if (e.altKey && e.key === "ArrowDown") { e.preventDefault(); move(p.id, 1); } } });
    const row = h("div", { class: "pl", "data-id": p.id }, grip,
      h("div", { class: "minw" }, nameEl("div", "n", p.name), h("div", { class: "u mono" }, hostPort(p.url))),
      h("span", { class: "chip" }, t("healthPath", p.health_path)));
    if (S.removing === p.id) {
      row.append(h("span", { class: "confirm" }, t("removeQ")),
        h("div", { class: "sidebtns" },
          h("button", { class: "btn red small", onclick: async () => { S.removing = null; await savePanels(S.panels.filter((x) => x.id !== p.id).map(toInput)); render(); } }, t("yesRemove")),
          h("button", { class: "btn small", onclick: () => { S.removing = null; render(); } }, t("cancel"))));
    } else {
      row.append(
        h("button", { class: "btn small", onclick: () => { S.editing = p.id; S.adding = false; S.error = null; render(); } }, t("edit")),
        h("button", { class: "btn small", onclick: () => { S.removing = p.id; render(); } }, t("remove")));
    }
    if (S.editing === p.id) row.append(editor(p, false));
    return row;
  }

  function panelsSection() {
    const sec = h("section", { class: "sec", id: "sec-panels" },
      h("h2", { class: "h" }, t("panels")),
      h("p", { class: "intro" }, t("panelsIntro")),
      ...S.panels.map(panelRow));
    if (S.adding) sec.append(h("div", { class: "pl" }, h("span"), editor(null, true)));
    sec.append(h("div", { class: "add-row" },
      h("button", { class: "btn", onclick: () => { S.adding = true; S.editing = null; S.error = null; render(); } }, t("addPanel")),
      h("button", { class: "btn", onclick: () => show("find") }, t("find"))));
    return sec;
  }

  /* ---- Other sections -------------------------------------------------------------------- */

  async function setPref(key, value) {
    const prefs = Object.assign({}, S.prefs, { [key]: value });
    try {
      S.prefs = await invoke("set_prefs", { prefs });
      if (L.info) L.info.prefs = S.prefs;
      toast(t("saved"));
    } catch (e) {
      toast(errorText(e), true);
    }
    render();
  }

  function generalSection() {
    const info = L.info || {};
    return h("section", { class: "sec", id: "sec-general" },
      h("h2", { class: "h small" }, t("general")),
      itemRow(t("startWithWindows"), t("startWithWindowsD"), toggle("pref-autostart", S.autostart, async (e) => {
        S.autostart = await invoke("set_autostart", { enabled: e.target.checked });
        render();
      }, t("startWithWindows"))),
      itemRow(t("keepPreviews"), t("keepPreviewsD"), [
        h("button", { class: "btn small", onclick: async () => { const n = await invoke("clear_previews"); L.previews = {}; toast(t("previewsCleared", n)); } }, t("clearPreviews")),
        toggle("pref-previews", S.prefs.keep_previews, (e) => setPref("keep_previews", e.target.checked), t("keepPreviews"))]),
      itemRow(t("logFiles"), t("logFilesD"), h("button", { class: "btn small", onclick: () => invoke("open_log_folder").catch((e) => toast(errorText(e), true)) }, t("openLogs")),
        h("div", { class: "path mono" }, info.log_dir || "")),
      itemRow(t("dataFolder"), null, null, h("div", { class: "path mono" }, info.data_dir || "")));
  }

  function notificationsSection() {
    return h("section", { class: "sec", id: "sec-notifications" },
      h("h2", { class: "h small" }, t("notifications")),
      itemRow(t("notifyDown"), t("notifyDownD"), toggle("pref-notify", S.prefs.notify, (e) => setPref("notify", e.target.checked), t("notifyDown"))));
  }

  function passwordsSection() {
    const side = [toggle("pref-passwords", S.prefs.save_passwords, (e) => setPref("save_passwords", e.target.checked), t("savePasswords"))];
    const clear = isWindows()
      ? h("button", { class: "btn small", onclick: async (e) => {
          e.currentTarget.disabled = true;
          try { await invoke("clear_saved_passwords"); toast(t("passwordsCleared")); } catch (err) { toast(errorText(err), true); }
          render();
        } }, t("clearPasswords"))
      : h("span", { class: "faint" }, t("windowsOnly"));
    return h("section", { class: "sec", id: "sec-passwords" },
      h("h2", { class: "h small" }, t("savedPasswords")),
      itemRow(t("savePasswords"), t("savePasswordsD"), side),
      itemRow(t("clearPasswords"), null, clear));
  }

  function aboutSection() {
    const info = L.info || {};
    return h("section", { class: "sec", id: "sec-about" },
      h("h2", { class: "h small" }, t("about")),
      h("div", { class: "about" }, h("img", { src: "img/icon.svg", alt: "" }),
        h("div", {},
          h("div", { class: "mono muted" }, t("version", info.version || "")),
          h("p", {}, t("aboutText")),
          h("p", { class: "muted" }, t("privacy")),
          h("p", { class: "muted" }, t("license")),
          h("button", { class: "btn solid", onclick: () => invoke("open_releases") }, t("checkVersions")))));
  }

  /* ---- Layout ---------------------------------------------------------------------------- */

  function renderNav() {
    const nav = document.getElementById("subnav");
    if (nav.childElementCount) return;
    nav.replaceChildren(...SECTIONS.map(([id, key]) => h("button", { "data-sec": id, onclick: () => {
      document.getElementById(id).scrollIntoView({ behavior: "smooth", block: "start" });
    } }, t(key))));
    const box = document.getElementById("sections");
    box.addEventListener("scroll", spy);
  }

  function spy() {
    const box = document.getElementById("sections");
    const top = box.getBoundingClientRect().top + 40;
    let current = SECTIONS[0][0];
    for (const [id] of SECTIONS) {
      const el = document.getElementById(id);
      if (el && el.getBoundingClientRect().top <= top) current = id;
    }
    if (box.scrollTop + box.clientHeight >= box.scrollHeight - 4) current = SECTIONS[SECTIONS.length - 1][0];
    for (const b of document.querySelectorAll("#subnav button")) b.setAttribute("aria-current", String(b.dataset.sec === current));
  }

  function render() {
    if (!S.prefs) return;
    renderNav();
    const box = document.getElementById("sections");
    const scroll = box.scrollTop;
    box.replaceChildren(panelsSection(), generalSection(), notificationsSection(), passwordsSection(), aboutSection());
    box.scrollTop = scroll;
    spy();
  }

  async function load() {
    try {
      const [panels, prefs, autostart] = await Promise.all([invoke("list_panels"), invoke("get_prefs"), invoke("get_autostart")]);
      S.panels = panels;
      S.prefs = prefs;
      S.autostart = autostart;
    } catch (e) {
      toast(errorText(e), true);
    }
  }

  window.settingsView = {
    async render() {
      await load();
      render();
    },
    async edit(id) {
      await load();
      S.editing = id; S.adding = false; S.error = null;
      render();
      const row = document.querySelector(`.pl[data-id="${CSS.escape(id)}"]`);
      if (row) row.scrollIntoView({ block: "center" });
    },
    async addNew() {
      await load();
      S.adding = true; S.editing = null; S.error = null;
      render();
      const ed = document.getElementById("ed-name");
      if (ed) ed.scrollIntoView({ block: "center" });
    },
  };
})();
