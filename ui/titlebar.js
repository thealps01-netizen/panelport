"use strict";

// Title bar of a panel window. It may only call the title-bar commands (capabilities/title-bar.json),
// all of which act on this window and its own panel.
let lastSig = "";

async function refresh() {
  let info = null;
  try {
    info = await invoke("bar_info");
  } catch (e) {
    console.error("bar_info", e);
    return;
  }
  if (!info) return;
  if (info.lang && LANG !== info.lang) {
    LANG = info.lang === "tr" ? "tr" : "en";
    document.documentElement.lang = LANG;
    applyStrings();
  }
  const sig = JSON.stringify(info);
  if (sig === lastSig) return;
  lastSig = sig;
  document.title = info.name;
  const name = document.getElementById("name");
  name.textContent = info.name;
  name.lang = nameLang(info.name);
  name.title = info.name;
  document.getElementById("url").textContent = hostPort(info.url);
  const dot = document.getElementById("dot");
  dot.className = `dot ${STATE_CLASS[info.state] || ""}`;
  dot.title = t(info.state);
  document.getElementById("who").setAttribute("aria-label", `${info.name}: ${t(info.state)}`);
}

(async function main() {
  document.getElementById("reload").append(icon("reload"));
  document.getElementById("reload").onclick = () => invoke("bar_reload");
  document.getElementById("back").onclick = () => invoke("bar_back");
  await refresh();
  applyStrings();
  wireTitlebar(document.getElementById("bar"));
  setInterval(refresh, 2000);
})();
