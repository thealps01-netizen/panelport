// Shared helpers for Panelport's bundled pages. No third-party code.
"use strict";

const STRINGS = {
  en: {
    panels: "Panels", find: "Find panels", settings: "Settings", addPanel: "Add panel",
    minimize: "Minimize", maximize: "Maximize", restore: "Restore", close: "Close",
    dockerRunning: "Docker Desktop is running", dockerStopped: "Docker is not running", dockerMissing: "Docker is not installed", dockerUnknown: "Checking Docker…",
    up: "Running", down: "Not reachable", error: "Server error", not_page: "Not a web page / no access", not_page_short: "No access", client_error: "Error", checking: "Checking…",
    downFor: (d) => `Down · ${d}`,
    listHead: (n, up) => (n === 1 ? `1 panel · ${up} running` : `${n} panels · ${up} running`),
    startContainers: "Start containers", startDocker: "Start Docker Desktop", starting: "Starting…",
    open: "Open", reload: "Refresh", logs: "Logs", edit: "Edit",
    container: (c) => `Container: ${c}`,
    noPreview: "A preview appears here after you open the panel once.",
    previewsOff: "Previews are switched off in Settings.",
    logsOf: (c) => `Log · ${c}`, logsEmpty: "The container has written nothing yet.", closeLogs: "Close", refreshLogs: "Refresh",
    notDocker: "This panel is not a Docker container on this PC, so there is no container log.",
    emptyTitle: "No panels yet",
    emptyText: "Panelport opens the web panels running on this computer — Docker dashboards, home-lab tools, anything with a local web page — each in its own window.",
    findNow: "Find panels", addByAddress: "Add by address",
    // find
    scanning: "Looking for web panels on this computer…",
    foundN: (n) => (n === 1 ? "1 panel found" : `${n} panels found`),
    foundNone: "No new web panels found",
    scannedAgo: (s) => `Found on this computer · scanned ${s}`,
    justNow: "just now", secondsAgo: (n) => `${n} s ago`, minutesAgo: (n) => `${n} min ago`,
    srcTitle: (t) => `Page title “${t}”`, srcDocker: (c) => `Docker container ${c}`, srcLocal: "Local server outside Docker",
    tagKnown: "Already added", tagDocker: "Docker", tagLocal: "Local",
    skipped: (n, list) => `${n === 1 ? "1 address" : `${n} addresses`} skipped: not a working web page (${list}). These are not added as panels.`,
    notAdded: (n, list) => (n === 1 ? `${list} no longer answers with a working web page, so it was not added.` : `${list} no longer answer with a working web page, so they were not added.`),
    selected: "Selected",
    willAdd: (n) => (n === 0 ? "Tick the panels you want to add." : n === 1 ? "1 new panel will be added:" : `${n} new panels will be added:`),
    addN: (n) => (n === 0 ? "Add panels" : n === 1 ? "Add 1 panel" : `Add ${n} panels`),
    rescan: "Scan again",
    scanNote: "Ports published by Docker and local servers that return an HTML page are scanned. Nothing is started.",
    added: (n) => (n === 1 ? "1 panel added." : `${n} panels added.`),
    // settings
    general: "General", notifications: "Notifications", savedPasswords: "Saved passwords", about: "About",
    panelsIntro: "Drag to reorder. The order is the same in the list and in the tray menu.",
    healthPath: (p) => `Health path: ${p}`,
    remove: "Remove", removeQ: "Remove this panel?", yesRemove: "Yes, remove", cancel: "Cancel", save: "Save",
    name: "Name", address: "Address", health: "Health check path", icon: "Icon", iconFavicon: "Site icon", iconMonogram: "Letter",
    newPanel: "New panel", moveUp: "Move up", moveDown: "Move down", dragHint: "Drag, or use Alt+Up and Alt+Down",
    startWithWindows: "Start with Windows", startWithWindowsD: "Panelport starts in the tray when you sign in and keeps watching your panels.",
    keepPreviews: "Keep previews", keepPreviewsD: "A still image of each panel, taken while its window is open, shown in the launcher. It can show whatever the panel shows and is stored only on this PC.",
    clearPreviews: "Delete previews", previewsCleared: (n) => (n === 1 ? "1 preview deleted." : `${n} previews deleted.`),
    logFiles: "Log files", logFilesD: "Useful when something goes wrong.", openLogs: "Open log folder",
    dataFolder: "Settings folder",
    notifyDown: "Notify when a panel goes down", notifyDownD: "A single failed check does not count; you are told after 10 seconds without an answer, and again when it is back.",
    savePasswords: "Save passwords", savePasswordsD: "Panel windows offer to save the passwords you type and fill them in next time. They are stored only on this PC, in Panelport's own folder, protected for your Windows account; they are not synced and not shared with Microsoft Edge.",
    clearPasswords: "Delete saved passwords", passwordsCleared: "Saved passwords deleted.", windowsOnly: "Available on Windows.",
    aboutText: "Panelport is an unofficial companion app that opens web panels running on this PC (for example Arcane, a Docker manager) in their own windows. It is not affiliated with or endorsed by the makers of any panel.",
    privacy: "No telemetry: Panelport only talks to the panel addresses you add and to Docker on this PC.",
    license: "Free and open source under the MIT License. Third-party licenses, including the fonts' SIL Open Font License, are in THIRD_PARTY_LICENSES.txt in the program folder.",
    checkVersions: "Check for new versions", version: (v) => `Version ${v}`,
    saved: "Saved.",
    noticeUpgraded: "Your settings were updated to the new format. A backup of the old file is kept next to it.",
    noticeCorrupt: "The settings file could not be read, so Panelport started fresh. The old file was kept as: ",
    noticeTooNew: "These settings come from a newer Panelport, so changes cannot be saved. Install the latest version from the releases page.",
    dismiss: "Dismiss",
    // down page
    downSince: (d) => `Not reachable · ${d}`, checkingNow: "Checking…",
    notPageTitle: "Not a web page", errorTitle: "Server error",
    leadStopped: (c) => `The container ${c} that runs this panel has stopped. Docker Desktop is running, so you can start it from here.`,
    leadDocker: "Docker is not running, so the containers behind this panel cannot run. Start Docker Desktop, then the panel comes back by itself.",
    leadPlain: "The panel's address does not answer. Panelport keeps trying and opens the panel as soon as it answers.",
    leadNotPage: "The address answers, but not with a web page; it is usually a program's interface that needs a key. Check the address and health check path in Settings, or remove this panel.",
    leadError: "The panel answers with a server error. Panelport keeps checking and shows the panel again when it recovers.",
    tryAgain: "Try again", editPanel: "Edit panel",
    engineOk: "Docker Desktop is running", engineOkD: (v) => (v ? `Engine ${v} answers` : "The engine answers"),
    engineOff: "Docker is not running", engineOffD: "Start Docker Desktop to bring the containers back",
    engineMissing: "Docker is not installed", engineMissingD: "Start the panel's program yourself",
    contStopped: (c) => `Container ${c} has stopped`, contRunning: (c) => `Container ${c} is running`,
    exitInfo: (code, at) => [code !== null && code !== undefined ? `Exit code ${code}` : "", at ? `stopped at ${at}` : ""].filter(Boolean).join(" · "),
    addrNo: "The panel's address does not answer", addrNotPage: "The address answers without a web page", addrError: "The address answers with a server error",
    addrD: (addr, detail) => `${addr} · ${detail} · checked every 5 s`,
    details: "Details", kAddress: "Address", kContainer: "Container", kCompose: "Compose", kLastOk: "Last answer",
    lastLog: "Last log lines", retrying: "Checking again every 5 seconds…",
    containersStarted: "Containers started. Waiting for the panel to answer…", dockerStarting: "Docker Desktop is starting. This can take a minute.",
    backToPanels: "Back to panels",
    minutes: (n) => (n === 1 ? "1 min" : `${n} min`), hours: (n) => `${n} h`, seconds: (n) => `${n} s`,
    minutesLong: (n) => (n === 1 ? "1 minute" : `${n} minutes`), hoursLong: (n) => (n === 1 ? "1 hour" : `${n} hours`), secondsLong: (n) => `${n} seconds`,
  },
  tr: {
    panels: "Paneller", find: "Panel bul", settings: "Ayarlar", addPanel: "Panel ekle",
    minimize: "Küçült", maximize: "Büyüt", restore: "Önceki boyut", close: "Kapat",
    dockerRunning: "Docker Desktop çalışıyor", dockerStopped: "Docker çalışmıyor", dockerMissing: "Docker kurulu değil", dockerUnknown: "Docker denetleniyor…",
    up: "Çalışıyor", down: "Ulaşılamıyor", error: "Sunucu hatası", not_page: "Web sayfası değil / erişim yok", not_page_short: "Erişim yok", client_error: "Hata", checking: "Denetleniyor…",
    downFor: (d) => `Kapalı · ${d}`,
    listHead: (n, up) => `${n} panel · ${up} çalışıyor`,
    startContainers: "Konteyneri başlat", startDocker: "Docker Desktop'ı başlat", starting: "Başlatılıyor…",
    open: "Aç", reload: "Yenile", logs: "Günlük", edit: "Düzenle",
    container: (c) => `Konteyner: ${c}`,
    noPreview: "Paneli bir kez açtığınızda önizlemesi burada görünür.",
    previewsOff: "Önizlemeler Ayarlar'da kapalı.",
    logsOf: (c) => `Günlük · ${c}`, logsEmpty: "Konteyner henüz bir şey yazmadı.", closeLogs: "Kapat", refreshLogs: "Yenile",
    notDocker: "Bu panel bu bilgisayarda bir Docker konteyneri değil, bu yüzden konteyner günlüğü yok.",
    emptyTitle: "Henüz panel yok",
    emptyText: "Panelport bu bilgisayarda çalışan web panellerini — Docker panoları, ev laboratuvarı araçları, yerel web sayfası olan her şey — kendi pencerelerinde açar.",
    findNow: "Panel bul", addByAddress: "Adresle ekle",
    scanning: "Bu bilgisayardaki web panelleri aranıyor…",
    foundN: (n) => `${n} panel bulundu`,
    foundNone: "Yeni web paneli bulunamadı",
    scannedAgo: (s) => `Bu bilgisayarda bulunanlar · ${s} tarandı`,
    justNow: "az önce", secondsAgo: (n) => `${n} sn önce`, minutesAgo: (n) => `${n} dk önce`,
    srcTitle: (t) => `Sayfa başlığı “${t}”`, srcDocker: (c) => `Docker konteyneri ${c}`, srcLocal: "Docker dışı yerel sunucu",
    tagKnown: "Zaten ekli", tagDocker: "Docker", tagLocal: "Yerel",
    skipped: (n, list) => `${n} adres atlandı: çalışan bir web sayfası değil (${list}). Bunlar panel olarak eklenmez.`,
    notAdded: (n, list) => `${list} artık çalışan bir web sayfasıyla yanıt vermiyor; eklenmedi.`,
    selected: "Seçilenler",
    willAdd: (n) => (n === 0 ? "Eklemek istediğiniz panelleri işaretleyin." : `${n} yeni panel eklenecek:`),
    addN: (n) => (n === 0 ? "Panel ekle" : `${n} paneli ekle`),
    rescan: "Yeniden tara",
    scanNote: "Docker'ın yayınladığı portlar ve HTML sayfası döndüren yerel sunucular taranır. Hiçbir şey başlatılmaz.",
    added: (n) => `${n} panel eklendi.`,
    general: "Genel", notifications: "Bildirimler", savedPasswords: "Kayıtlı şifreler", about: "Hakkında",
    panelsIntro: "Sürükleyerek sıralayın. Sıra, listede ve bildirim alanı menüsünde aynıdır.",
    healthPath: (p) => `Sağlık yolu: ${p}`,
    remove: "Kaldır", removeQ: "Bu panel kaldırılsın mı?", yesRemove: "Evet, kaldır", cancel: "Vazgeç", save: "Kaydet",
    name: "Ad", address: "Adres", health: "Sağlık denetimi yolu", icon: "Simge", iconFavicon: "Site simgesi", iconMonogram: "Harf",
    newPanel: "Yeni panel", moveUp: "Yukarı taşı", moveDown: "Aşağı taşı", dragHint: "Sürükleyin ya da Alt+Yukarı ve Alt+Aşağı kullanın",
    startWithWindows: "Windows ile başlat", startWithWindowsD: "Oturum açınca Panelport bildirim alanında başlar ve panellerinizi izler.",
    keepPreviews: "Önizlemeleri sakla", keepPreviewsD: "Her panelin, penceresi açıkken alınan bir görüntüsü başlatıcıda gösterilir. Panelin gösterdiği her şeyi içerebilir ve yalnızca bu bilgisayarda saklanır.",
    clearPreviews: "Önizlemeleri sil", previewsCleared: (n) => `${n} önizleme silindi.`,
    logFiles: "Günlük dosyaları", logFilesD: "Bir şeyler ters gittiğinde işe yarar.", openLogs: "Günlük klasörünü aç",
    dataFolder: "Ayarlar klasörü",
    notifyDown: "Panel kapanınca bildir", notifyDownD: "Tek bir başarısız kontrol sayılmaz; 10 saniye yanıt yoksa bildirilir, geri gelince yeniden bildirilir.",
    savePasswords: "Şifreleri kaydet", savePasswordsD: "Panel pencerelerinde girdiğiniz şifreler kaydedilir ve sonraki sefer doldurulur. Yalnızca bu bilgisayarda, Panelport'un klasöründe, Windows hesabınıza korumalı saklanır; eşitlenmez ve Microsoft Edge ile paylaşılmaz.",
    clearPasswords: "Kayıtlı şifreleri sil", passwordsCleared: "Kayıtlı şifreler silindi.", windowsOnly: "Windows'ta kullanılabilir.",
    aboutText: "Panelport, bu bilgisayarda çalışan web panellerini (örneğin bir Docker yöneticisi olan Arcane) kendi pencerelerinde açan resmi olmayan bir yardımcı uygulamadır. Hiçbir panelin yapımcısıyla bağlantılı değildir ve onlar tarafından onaylanmamıştır.",
    privacy: "Telemetri yok: Panelport yalnızca eklediğiniz panel adresleriyle ve bu bilgisayardaki Docker ile konuşur.",
    license: "MIT Lisansı altında özgür ve açık kaynaklıdır. Yazı tiplerinin SIL Open Font License metni dahil üçüncü taraf lisansları program klasöründeki THIRD_PARTY_LICENSES.txt dosyasındadır.",
    checkVersions: "Yeni sürümleri denetle", version: (v) => `Sürüm ${v}`,
    saved: "Kaydedildi.",
    noticeUpgraded: "Ayarlarınız yeni biçime güncellendi. Eski dosyanın yedeği yanında duruyor.",
    noticeCorrupt: "Ayar dosyası okunamadı, Panelport baştan başladı. Eski dosya şu adla saklandı: ",
    noticeTooNew: "Bu ayarlar daha yeni bir Panelport'tan geliyor, değişiklikler kaydedilemez. Sürümler sayfasından en yeni sürümü kurun.",
    dismiss: "Kapat",
    downSince: (d) => `Ulaşılamıyor · ${d}`, checkingNow: "Denetleniyor…",
    notPageTitle: "Web sayfası değil", errorTitle: "Sunucu hatası",
    leadStopped: (c) => `Bu paneli çalıştıran ${c} konteyneri durdu. Docker Desktop çalışıyor, konteyneri buradan başlatabilirsiniz.`,
    leadDocker: "Docker çalışmıyor, bu yüzden panelin konteynerleri çalışamaz. Docker Desktop'ı başlatın; panel kendiliğinden geri gelir.",
    leadPlain: "Panelin adresi yanıt vermiyor. Panelport denemeyi sürdürüyor ve panel yanıt verir vermez açıyor.",
    leadNotPage: "Adres yanıt veriyor ama bir web sayfasıyla değil; genellikle anahtar isteyen bir program arayüzü. Ayarlar'da adresi ve sağlık denetimi yolunu denetleyin ya da bu paneli kaldırın.",
    leadError: "Panel sunucu hatasıyla yanıt veriyor. Panelport denetlemeyi sürdürüyor ve panel düzelince yeniden gösteriyor.",
    tryAgain: "Tekrar dene", editPanel: "Paneli düzenle",
    engineOk: "Docker Desktop çalışıyor", engineOkD: (v) => (v ? `Motor ${v} yanıt veriyor` : "Motor yanıt veriyor"),
    engineOff: "Docker çalışmıyor", engineOffD: "Konteynerlerin geri gelmesi için Docker Desktop'ı başlatın",
    engineMissing: "Docker kurulu değil", engineMissingD: "Panelin programını kendiniz başlatın",
    contStopped: (c) => `${c} konteyneri durmuş`, contRunning: (c) => `${c} konteyneri çalışıyor`,
    exitInfo: (code, at) => [code !== null && code !== undefined ? `Çıkış kodu ${code}` : "", at ? `${at}'de durdu` : ""].filter(Boolean).join(" · "),
    addrNo: "Panel adresi yanıt vermiyor", addrNotPage: "Adres web sayfası olmadan yanıt veriyor", addrError: "Adres sunucu hatasıyla yanıt veriyor",
    addrD: (addr, detail) => `${addr} · ${detail} · her 5 sn denenir`,
    details: "Ayrıntılar", kAddress: "Adres", kContainer: "Konteyner", kCompose: "Compose", kLastOk: "Son yanıt",
    lastLog: "Son günlük satırları", retrying: "Her 5 saniyede bir yeniden denetleniyor…",
    containersStarted: "Konteynerler başlatıldı. Panelin yanıt vermesi bekleniyor…", dockerStarting: "Docker Desktop başlıyor. Bu bir dakika sürebilir.",
    backToPanels: "Panellere dön",
    minutes: (n) => `${n} dk`, hours: (n) => `${n} sa`, seconds: (n) => `${n} sn`,
    minutesLong: (n) => `${n} dakika`, hoursLong: (n) => `${n} saat`, secondsLong: (n) => `${n} saniye`,
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
  // from driving Panelport's commands through it (CSP frame-ancestors and the native top-level
  // guard block this too).
  if (window.top !== window.self) return Promise.reject({ message: "Panelport pages cannot run inside a frame." });
  const internals = window.__TAURI_INTERNALS__;
  if (!internals) return Promise.reject({ message: "Panelport's bridge is not available on this page." });
  return internals.invoke(cmd, args || {});
}

function errorText(e) {
  if (e && typeof e === "object" && "message" in e) return String(e.message);
  return String(e);
}

/** Tiny element builder: h("div", {class: "x", onclick: f}, child, "text"). No style attributes (CSP). */
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

/** Panel names are mostly not Turkish words: uppercase them in English so "Arcane" never becomes "ARCANE" with a dotted İ problem. */
function nameLang(name) {
  return /[çğıöşüÇĞİÖŞÜ]/.test(name || "") ? "tr" : "en";
}

function nameEl(tag, cls, name) {
  return h(tag, { class: cls, lang: nameLang(name), title: name }, name);
}

const STATE_CLASS = { up: "s-up", down: "s-down", error: "s-warn", not_page: "s-warn", client_error: "s-warn", checking: "s-checking" };
function stateEl(state, extra, big) {
  return h("span", { class: `state ${STATE_CLASS[state] || "s-checking"}${big ? " lg" : ""}` }, h("i"), extra || t(state));
}

function hostPort(url) {
  try { const u = new URL(url); return u.host + (u.pathname !== "/" ? u.pathname : ""); } catch { return url; }
}

/** "4 min" style duration since `ms` (milliseconds since 1970). */
function since(ms, long) {
  if (!ms) return "";
  const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
  if (s < 60) return long ? t("secondsLong", s) : t("seconds", s);
  const m = Math.round(s / 60);
  if (m < 60) return long ? t("minutesLong", m) : t("minutes", m);
  const hr = Math.round(m / 60);
  return long ? t("hoursLong", hr) : t("hours", hr);
}

/** Technical check details come from the app in English; the common ones read in Turkish too. */
const DETAIL_TR = { "connection refused": "bağlantı reddedildi", "timeout": "zaman aşımı", "host not found": "ana makine bulunamadı", "no content type": "içerik türü yok" };
function detailText(d) {
  if (LANG !== "tr" || !d) return d || "";
  return d.split(", ").map((part) => DETAIL_TR[part] || part).join(", ");
}

function clock(ms) {
  if (!ms) return "";
  return new Date(ms).toLocaleTimeString(LANG === "tr" ? "tr-TR" : "en-GB", { hour: "2-digit", minute: "2-digit" });
}

const ICONS = {
  open: '<svg viewBox="0 0 24 24"><path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/></svg>',
  reload: '<svg viewBox="0 0 24 24"><path d="M20 11a8 8 0 1 0-2.3 5.7M20 5v6h-6"/></svg>',
  logs: '<svg viewBox="0 0 24 24"><path d="M5 4h14v16H5zM9 9h6M9 13h6M9 17h3"/></svg>',
  edit: '<svg viewBox="0 0 24 24"><path d="M4 20h4L19 9l-4-4L4 16z"/></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg>',
  check: '<svg viewBox="0 0 20 20"><path d="m4 10 4 4 8-8"/></svg>',
  cross: '<svg viewBox="0 0 20 20"><path d="m5 5 10 10M15 5 5 15"/></svg>',
  clock: '<svg viewBox="0 0 20 20"><circle cx="10" cy="10" r="7"/><path d="M10 6v4l3 2"/></svg>',
  min: '<svg viewBox="0 0 12 12"><path d="M2 6h8"/></svg>',
  max: '<svg viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8" rx="1.5"/></svg>',
  restore: '<svg viewBox="0 0 12 12"><rect x="2" y="3.6" width="6.4" height="6.4" rx="1.2"/><path d="M4.2 3.6V3a1 1 0 0 1 1-1H9a1 1 0 0 1 1 1v3.8a1 1 0 0 1-1 1h-.6"/></svg>',
  x: '<svg viewBox="0 0 12 12"><path d="m2.5 2.5 7 7m0-7-7 7"/></svg>',
};

/** Static, trusted SVG markup from ICONS (never page or panel data). */
function icon(name) {
  const tpl = document.createElement("template");
  tpl.innerHTML = ICONS[name];
  return tpl.content.firstChild;
}

let toastTimer = null;
function toast(text, isError) {
  document.querySelectorAll(".toast").forEach((x) => x.remove());
  const el = h("div", { class: `toast${isError ? " err" : ""}`, role: isError ? "alert" : "status" }, text);
  document.body.append(el);
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.remove(), isError ? 7000 : 3200);
}

/* ---- Title bar: drag, double-click, caption buttons ------------------------------------------ */

/** Wires a title bar: mouse-down on empty bar space drags the window (Aero Snap included),
 *  double-click toggles maximize; the caption buttons work by click everywhere, and on Windows
 *  the native hit area over them (chrome.rs) reports hover and press through __ppCaption. */
function wireTitlebar(bar) {
  const caption = h("div", { class: "caption" },
    h("button", { class: "min", "aria-label": t("minimize"), title: t("minimize"), tabindex: "-1", onclick: () => invoke("win_minimize") }, icon("min")),
    h("button", { class: "max", "aria-label": t("maximize"), title: t("maximize"), tabindex: "-1", onclick: () => invoke("win_toggle_maximize").then(setMax) },
      h("span", { class: "maximize" }, icon("max")), h("span", { class: "restore" }, icon("restore"))),
    h("button", { class: "close", "aria-label": t("close"), title: t("close"), tabindex: "-1", onclick: () => invoke("win_close") }, icon("x")));
  bar.append(caption);
  bar.addEventListener("mousedown", (e) => {
    if (e.button !== 0) return;
    if (e.target.closest("button, input, select, a, [data-nodrag]")) return;
    if (e.detail === 2) {
      invoke("win_toggle_maximize").then(setMax).catch(() => {});
    } else {
      invoke("win_start_drag").catch(() => {});
    }
  });
  const btn = { min: caption.querySelector(".min"), max: caption.querySelector(".max"), close: caption.querySelector(".close") };
  window.__ppCaption = (which, state) => {
    for (const [k, el] of Object.entries(btn)) {
      el.classList.toggle("hot", k === which && state === "hover");
      el.classList.toggle("press", k === which && state === "press");
    }
  };
  window.__ppMax = setMax;
  const sync = () => invoke("win_is_maximized").then(setMax).catch(() => {});
  window.addEventListener("resize", () => { clearTimeout(wireTitlebar.t); wireTitlebar.t = setTimeout(sync, 120); });
  sync();
}

function setMax(on) {
  document.body.classList.toggle("maximized", !!on);
  const b = document.querySelector(".caption .max");
  if (b) { b.title = on ? t("restore") : t("maximize"); b.setAttribute("aria-label", b.title); }
}

function applyStrings(root) {
  for (const el of (root || document).querySelectorAll("[data-t]")) el.textContent = t(el.dataset.t);
  for (const el of (root || document).querySelectorAll("[data-t-label]")) {
    el.setAttribute("aria-label", t(el.dataset.tLabel));
    el.title = t(el.dataset.tLabel);
  }
}
