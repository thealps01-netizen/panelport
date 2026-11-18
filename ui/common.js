// Shared helpers for the bundled pages. No third-party code.
"use strict";

const STRINGS = {
  en: {
    tagline: "Your local web panels",
    find: "Find panels",
    settings: "Settings",
    open: "Open",
    up: "Running",
    down: "Not reachable",
    error: "Server error",
    not_page: "Not a web page / no access",
    checking: "Checking…",
    nPanels: (n) => (n === 1 ? "1 panel" : `${n} panels`),
    nUp: (n) => `${n} running`,
    nDown: (n) => `${n} not reachable`,
    emptyTitle: "No panels yet",
    emptyText: "Panelport opens the web panels running on this computer — Docker dashboards, home-lab tools, anything with a local web page — in their own windows.",
    addManually: "Add by address",
    findTitle: "Find panels on this computer",
    findIntro: "Panelport looked at ports published by Docker containers and at local web servers. Tick the ones you want.",
    searching: "Looking for web panels on this computer…",
    noneFound: "No web panels were found. Start the panel's program, or add it by address in Settings.",
    addSelected: (n) => (n === 1 ? "Add 1 panel" : `Add ${n} panels`),
    cancel: "Cancel",
    searchAgain: "Search again",
    fromDocker: (c) => `Docker: ${c}`,
    port: (p) => `Port ${p}`,
    noticeUpgraded: "Your settings were updated to the new format. A backup of the old file is kept next to it.",
    noticeCorrupt: "The settings file could not be read, so Panelport started fresh. The old file was kept as: ",
    noticeTooNew: "These settings come from a newer Panelport, so changes cannot be saved. Install the latest version from the releases page.",
    dismiss: "Dismiss",
    // settings
    tabPanels: "Panels",
    tabGeneral: "General",
    tabAbout: "About",
    name: "Name",
    address: "Address",
    healthPath: "Health check path",
    icon: "Icon",
    iconFavicon: "Site icon",
    iconMonogram: "Letter",
    moveUp: "Move up",
    moveDown: "Move down",
    remove: "Remove",
    addPanel: "Add panel",
    save: "Save changes",
    saved: "Saved.",
    unsaved: "You have unsaved changes.",
    startWithWindows: "Start with Windows",
    startWithWindowsHint: "Panelport starts in the tray when you sign in and keeps watching your panels.",
    logs: "Log files",
    logsHint: "Useful when something goes wrong.",
    openLogs: "Open log folder",
    dataFolder: "Settings folder",
    aboutText: "Panelport is an unofficial companion app that opens locally hosted web panels (for example Arcane, a Docker manager) as native windows. It is not affiliated with or endorsed by the makers of any panel.",
    privacy: "No telemetry: Panelport only talks to the panel addresses you add and to Docker on this computer.",
    license: "Free and open source under the MIT License. Third-party licenses are in THIRD_PARTY_LICENSES.txt in the program folder.",
    releases: "Check for new versions",
    version: (v) => `Version ${v}`,
    newPanel: "New panel",
    // down page
    connecting: (n) => `Connecting to ${n}…`,
    cantReach: (n) => `Can't reach ${n}`,
    serverError: (n) => `${n} answers with an error`,
    notPageTitle: (n) => `${n} is not a web page Panelport can show`,
    notPageIntro: "The address answers, but not with a web page — usually a program's interface that needs a key. Check the address and health check path in Settings, or remove this panel.",
    downIntro: "Panelport keeps trying and opens the panel as soon as it answers.",
    dockerEngine: "Docker engine",
    engineRunning: "Running",
    engineStopped: "Not running",
    engineMissing: "Not installed",
    containers: "Containers",
    container: (n) => `Container ${n}`,
    panelAddress: "Panel address",
    noAnswer: "No answer",
    startDocker: "Start Docker Desktop",
    startContainers: "Start containers",
    starting: "Starting…",
    tryAgain: "Try again",
    editPanel: "Edit panel",
    retrying: "Checking again every few seconds…",
    dockerStarting: "Docker Desktop is starting. This can take a minute.",
    containersStarted: "Containers started. Waiting for the panel to answer…",
    stateRunning: "running",
  },
  tr: {
    tagline: "Yerel web panelleriniz",
    find: "Panel bul",
    settings: "Ayarlar",
    open: "Aç",
    up: "Çalışıyor",
    down: "Ulaşılamıyor",
    error: "Sunucu hatası",
    not_page: "Web sayfası değil / erişim yok",
    checking: "Denetleniyor…",
    nPanels: (n) => `${n} panel`,
    nUp: (n) => `${n} çalışıyor`,
    nDown: (n) => `${n} ulaşılamıyor`,
    emptyTitle: "Henüz panel yok",
    emptyText: "Panelport bu bilgisayarda çalışan web panellerini — Docker panoları, ev laboratuvarı araçları, yerel web sayfası olan her şey — kendi pencerelerinde açar.",
    addManually: "Adresle ekle",
    findTitle: "Bu bilgisayardaki panelleri bul",
    findIntro: "Panelport, Docker kapsayıcılarının yayınladığı bağlantı noktalarına ve yerel web sunucularına baktı. İstediklerinizi işaretleyin.",
    searching: "Bu bilgisayardaki web panelleri aranıyor…",
    noneFound: "Web paneli bulunamadı. Panelin programını başlatın ya da Ayarlar'dan adresle ekleyin.",
    addSelected: (n) => `${n} paneli ekle`,
    cancel: "Vazgeç",
    searchAgain: "Yeniden ara",
    fromDocker: (c) => `Docker: ${c}`,
    port: (p) => `Bağlantı noktası ${p}`,
    noticeUpgraded: "Ayarlarınız yeni biçime güncellendi. Eski dosyanın yedeği yanında duruyor.",
    noticeCorrupt: "Ayar dosyası okunamadı, Panelport baştan başladı. Eski dosya şu adla saklandı: ",
    noticeTooNew: "Bu ayarlar daha yeni bir Panelport'tan geliyor, değişiklikler kaydedilemez. Sürümler sayfasından en yeni sürümü kurun.",
    dismiss: "Kapat",
    tabPanels: "Paneller",
    tabGeneral: "Genel",
    tabAbout: "Hakkında",
    name: "Ad",
    address: "Adres",
    healthPath: "Sağlık denetimi yolu",
    icon: "Simge",
    iconFavicon: "Site simgesi",
    iconMonogram: "Harf",
    moveUp: "Yukarı taşı",
    moveDown: "Aşağı taşı",
    remove: "Kaldır",
    addPanel: "Panel ekle",
    save: "Değişiklikleri kaydet",
    saved: "Kaydedildi.",
    unsaved: "Kaydedilmemiş değişiklikler var.",
    startWithWindows: "Windows ile başlat",
    startWithWindowsHint: "Oturum açtığınızda Panelport tepside başlar ve panellerinizi izlemeyi sürdürür.",
    logs: "Günlük dosyaları",
    logsHint: "Bir şeyler ters gittiğinde işe yarar.",
    openLogs: "Günlük klasörünü aç",
    dataFolder: "Ayarlar klasörü",
    aboutText: "Panelport, yerelde barındırılan web panellerini (örneğin bir Docker yöneticisi olan Arcane) yerel pencerelerde açan resmi olmayan bir yardımcı uygulamadır. Hiçbir panelin yapımcısıyla bağlantılı değildir ve onlar tarafından onaylanmamıştır.",
    privacy: "Telemetri yok: Panelport yalnızca eklediğiniz panel adresleriyle ve bu bilgisayardaki Docker ile konuşur.",
    license: "MIT Lisansı altında özgür ve açık kaynaklıdır. Üçüncü taraf lisansları program klasöründeki THIRD_PARTY_LICENSES.txt dosyasındadır.",
    releases: "Yeni sürümleri denetle",
    version: (v) => `Sürüm ${v}`,
    newPanel: "Yeni panel",
    connecting: (n) => `${n} paneline bağlanılıyor…`,
    cantReach: (n) => `${n} paneline ulaşılamıyor`,
    serverError: (n) => `${n} hata ile yanıt veriyor`,
    notPageTitle: (n) => `${n}, Panelport'un gösterebileceği bir web sayfası değil`,
    notPageIntro: "Adres yanıt veriyor ama bir web sayfasıyla değil; genellikle anahtar isteyen bir program arayüzü. Ayarlar'da adresi ve sağlık denetimi yolunu denetleyin ya da bu paneli kaldırın.",
    downIntro: "Panelport denemeyi sürdürüyor ve panel yanıt verir vermez açıyor.",
    dockerEngine: "Docker motoru",
    engineRunning: "Çalışıyor",
    engineStopped: "Çalışmıyor",
    engineMissing: "Kurulu değil",
    containers: "Kapsayıcılar",
    container: (n) => `Kapsayıcı ${n}`,
    panelAddress: "Panel adresi",
    noAnswer: "Yanıt yok",
    startDocker: "Docker Desktop'ı başlat",
    startContainers: "Kapsayıcıları başlat",
    starting: "Başlatılıyor…",
    tryAgain: "Yeniden dene",
    editPanel: "Paneli düzenle",
    retrying: "Birkaç saniyede bir yeniden denetleniyor…",
    dockerStarting: "Docker Desktop başlıyor. Bu bir dakika sürebilir.",
    containersStarted: "Kapsayıcılar başlatıldı. Panelin yanıt vermesi bekleniyor…",
    stateRunning: "çalışıyor",
  },
};

let LANG = "en";
function t(key, ...args) {
  const table = STRINGS[LANG] || STRINGS.en;
  const v = key in table ? table[key] : STRINGS.en[key];
  return typeof v === "function" ? v(...args) : v;
}

function invoke(cmd, args) {
  // Bundled pages never run inside a frame; refusing here keeps a panel page that frames one
  // from driving Panelport's commands through it (CSP frame-ancestors also blocks this).
  if (window.top !== window.self) return Promise.reject({ message: "Panelport pages cannot run inside a frame." });
  const internals = window.__TAURI_INTERNALS__;
  if (!internals) return Promise.reject({ message: "Panelport's bridge is not available on this page." });
  return internals.invoke(cmd, args || {});
}

function errorText(e) {
  if (e && typeof e === "object" && "message" in e) return String(e.message);
  return String(e);
}

/** Tiny element builder: h("div", {class: "x", onclick: f}, child, "text"). */
function h(tag, attrs, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === null || v === undefined || v === false) continue;
    if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k === "class") el.className = v;
    else el.setAttribute(k, v === true ? "" : v);
  }
  for (const c of children.flat()) {
    if (c === null || c === undefined || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

function hue(text) {
  let x = 0;
  for (const ch of text) x = (x * 31 + ch.codePointAt(0)) >>> 0;
  return x % 360;
}

/** Favicon if cached and wanted, otherwise a coloured monogram. */
function avatar(panel) {
  if (panel.icon_data) {
    return h("div", { class: "avatar has-img" }, h("img", { src: panel.icon_data, alt: "" }));
  }
  const letter = (panel.name.trim()[0] || "?").toLocaleUpperCase(LANG);
  const el = h("div", { class: "avatar", "aria-hidden": "true" }, letter);
  const hh = hue(panel.name);
  el.style.background = `linear-gradient(135deg, hsl(${hh} 70% 55%), hsl(${(hh + 40) % 360} 65% 42%))`;
  return el;
}

function statusLabel(state) {
  return { up: t("up"), down: t("down"), error: t("error"), not_page: t("not_page"), checking: t("checking") }[state] || state;
}

function statusEl(state) {
  return h("span", { class: `status ${state}` }, h("span", { class: `dot ${state}` }), statusLabel(state));
}

async function initLang() {
  const info = await invoke("app_info");
  LANG = info.lang === "tr" ? "tr" : "en";
  document.documentElement.lang = LANG;
  for (const el of document.querySelectorAll("[data-t]")) el.textContent = t(el.dataset.t);
  for (const el of document.querySelectorAll("[data-t-title]")) el.title = t(el.dataset.tTitle);
  return info;
}

const ICONS = {
  search: '<svg class="svg-i" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>',
  gear: '<svg class="svg-i" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>',
  arrow: '<svg class="svg-i" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M7 17 17 7M8 7h9v9"/></svg>',
  up: '<svg class="svg-i" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="m6 15 6-6 6 6"/></svg>',
  down: '<svg class="svg-i" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="m6 9 6 6 6-6"/></svg>',
  trash: '<svg class="svg-i" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6"/></svg>',
  plus: '<svg class="svg-i" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>',
};

/** Static, trusted SVG markup from ICONS (never page or panel data). */
function icon(name) {
  const tpl = document.createElement("template");
  tpl.innerHTML = ICONS[name];
  return tpl.content.firstChild;
}
