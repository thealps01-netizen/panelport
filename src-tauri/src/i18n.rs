//! UI language: Turkish when the OS UI language is Turkish, English otherwise.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Tr,
}

pub fn detect() -> Lang {
    if let Ok(forced) = std::env::var("PANELPORT_LANG") {
        return from_tag(&forced);
    }
    sys_locale::get_locale()
        .map(|l| from_tag(&l))
        .unwrap_or(Lang::En)
}

pub fn from_tag(tag: &str) -> Lang {
    let t = tag.to_ascii_lowercase();
    if t == "tr" || t.starts_with("tr-") || t.starts_with("tr_") {
        Lang::Tr
    } else {
        Lang::En
    }
}

/// Strings shown by the native side (tray menu, notifications, dialogs).
pub struct T {
    pub open_launcher: &'static str,
    pub find_panels: &'static str,
    pub settings: &'static str,
    pub start_with_windows: &'static str,
    pub quit: &'static str,
    pub no_panels: &'static str,
    pub up: &'static str,
    pub down: &'static str,
    pub error: &'static str,
    pub not_page: &'static str,
    pub checking: &'static str,
    pub tray_all_up: &'static str,
    pub tray_some_down: &'static str,
    pub tray_all_down: &'static str,
    pub panel_down_title: &'static str,
    pub panel_down_body: &'static str,
    pub panel_up_title: &'static str,
    pub panel_up_body: &'static str,
    pub tray_hint_title: &'static str,
    pub tray_hint_body: &'static str,
    pub launcher_title: &'static str,
    pub settings_title: &'static str,
}

const EN: T = T {
    open_launcher: "Open Panelport",
    find_panels: "Find panels…",
    settings: "Settings…",
    start_with_windows: "Start with Windows",
    quit: "Quit Panelport",
    no_panels: "No panels yet",
    up: "running",
    down: "not reachable",
    error: "server error",
    not_page: "not a web page / no access",
    checking: "checking…",
    tray_all_up: "All panels are running",
    tray_some_down: "Some panels are not reachable",
    tray_all_down: "No panel is reachable",
    panel_down_title: "{name} is not reachable",
    panel_down_body: "Panelport keeps checking and will tell you when it is back.",
    panel_up_title: "{name} is back",
    panel_up_body: "The panel is reachable again.",
    tray_hint_title: "Panelport is still running",
    tray_hint_body:
        "It keeps watching your panels from the system tray. Right-click the tray icon to quit.",
    launcher_title: "Panelport",
    settings_title: "Panelport Settings",
};

const TR: T = T {
    open_launcher: "Panelport'u aç",
    find_panels: "Panel bul…",
    settings: "Ayarlar…",
    start_with_windows: "Windows ile başlat",
    quit: "Panelport'tan çık",
    no_panels: "Henüz panel yok",
    up: "çalışıyor",
    down: "ulaşılamıyor",
    error: "sunucu hatası",
    not_page: "web sayfası değil / erişim yok",
    checking: "denetleniyor…",
    tray_all_up: "Tüm paneller çalışıyor",
    tray_some_down: "Bazı panellere ulaşılamıyor",
    tray_all_down: "Hiçbir panele ulaşılamıyor",
    panel_down_title: "{name}: panele ulaşılamıyor",
    panel_down_body: "Panelport denetlemeyi sürdürüyor; geri geldiğinde haber verecek.",
    panel_up_title: "{name}: panel yeniden çalışıyor",
    panel_up_body: "Panele yeniden ulaşılabiliyor.",
    tray_hint_title: "Panelport çalışmayı sürdürüyor",
    tray_hint_body: "Panellerinizi sistem tepsisinden izlemeye devam ediyor. Çıkmak için tepsi simgesine sağ tıklayın.",
    launcher_title: "Panelport",
    settings_title: "Panelport Ayarları",
};

pub fn t(lang: Lang) -> &'static T {
    match lang {
        Lang::En => &EN,
        Lang::Tr => &TR,
    }
}

/// User-facing errors: one plain sentence with the cause and the next step. `{log}` is replaced
/// with the log folder for errors whose details only the log has.
#[derive(Debug, Clone, Copy)]
pub enum ErrKind {
    InvalidUrl,
    InvalidName,
    InvalidHealthPath,
    ReadOnly,
    SaveFailed,
    UnknownPanel,
    OpenFailed,
    DockerMissing,
    DockerDesktopMissing,
    DockerStartFailed,
    ContainersStartFailed,
    NoContainers,
    Busy,
    LogFolderFailed,
}

pub fn err(lang: Lang, e: ErrKind) -> &'static str {
    use ErrKind::*;
    match (lang, e) {
        (Lang::En, InvalidUrl) => "That is not a web address Panelport can open. Enter one like http://localhost:3552.",
        (Lang::Tr, InvalidUrl) => "Bu, Panelport'un açabileceği bir web adresi değil. http://localhost:3552 gibi bir adres girin.",
        (Lang::En, InvalidName) => "Every panel needs a name. Enter a short name of up to 60 characters.",
        (Lang::Tr, InvalidName) => "Her panelin bir adı olmalı. En çok 60 karakterlik kısa bir ad girin.",
        (Lang::En, InvalidHealthPath) => "The health check path must start with /. Enter / to check the start page.",
        (Lang::Tr, InvalidHealthPath) => "Sağlık denetimi yolu / ile başlamalı. Başlangıç sayfasını denetlemek için / girin.",
        (Lang::En, ReadOnly) => "These settings were saved by a newer Panelport, so this version cannot change them. Install the latest Panelport from the releases page.",
        (Lang::Tr, ReadOnly) => "Bu ayarlar daha yeni bir Panelport sürümüyle kaydedilmiş, bu sürüm onları değiştiremez. Sürümler sayfasından en yeni Panelport'u kurun.",
        (Lang::En, SaveFailed) => "Panelport could not save its settings. Check that the disk is not full and try again; details are in the log ({log}).",
        (Lang::Tr, SaveFailed) => "Panelport ayarlarını kaydedemedi. Diskin dolu olmadığını denetleyip yeniden deneyin; ayrıntılar günlükte ({log}).",
        (Lang::En, UnknownPanel) => "This panel no longer exists. Close this window and open the panel again from Panelport.",
        (Lang::Tr, UnknownPanel) => "Bu panel artık yok. Pencereyi kapatıp paneli Panelport'tan yeniden açın.",
        (Lang::En, OpenFailed) => "The panel window could not be opened. Try again; details are in the log ({log}).",
        (Lang::Tr, OpenFailed) => "Panel penceresi açılamadı. Yeniden deneyin; ayrıntılar günlükte ({log}).",
        (Lang::En, DockerMissing) => "Docker is not installed on this computer. Install Docker Desktop, or start the panel's program yourself.",
        (Lang::Tr, DockerMissing) => "Bu bilgisayarda Docker kurulu değil. Docker Desktop'ı kurun ya da panelin programını kendiniz başlatın.",
        (Lang::En, DockerDesktopMissing) => "Panelport could not find Docker Desktop. Start Docker yourself, then press Try again.",
        (Lang::Tr, DockerDesktopMissing) => "Panelport Docker Desktop'ı bulamadı. Docker'ı kendiniz başlatıp Yeniden dene'ye basın.",
        (Lang::En, DockerStartFailed) => "Docker Desktop could not be started. Start it from the Start menu, then press Try again; details are in the log ({log}).",
        (Lang::Tr, DockerStartFailed) => "Docker Desktop başlatılamadı. Başlat menüsünden açıp Yeniden dene'ye basın; ayrıntılar günlükte ({log}).",
        (Lang::En, ContainersStartFailed) => "The containers could not be started. Open Docker Desktop to see why, then press Try again; details are in the log ({log}).",
        (Lang::Tr, ContainersStartFailed) => "Kapsayıcılar başlatılamadı. Nedenini görmek için Docker Desktop'ı açın, sonra Yeniden dene'ye basın; ayrıntılar günlükte ({log}).",
        (Lang::En, NoContainers) => "No Docker container on this computer publishes this panel's port. Start the panel's program yourself, then press Try again.",
        (Lang::Tr, NoContainers) => "Bu bilgisayarda bu panelin bağlantı noktasını yayınlayan bir Docker kapsayıcısı yok. Panelin programını kendiniz başlatıp Yeniden dene'ye basın.",
        (Lang::En, Busy) => "Panelport is already starting something. Wait a moment, then try again.",
        (Lang::Tr, Busy) => "Panelport zaten bir şeyi başlatıyor. Biraz bekleyip yeniden deneyin.",
        (Lang::En, LogFolderFailed) => "The log folder could not be opened. Open it yourself: {log}",
        (Lang::Tr, LogFolderFailed) => "Günlük klasörü açılamadı. Klasörü kendiniz açın: {log}",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_tags() {
        assert_eq!(from_tag("tr-TR"), Lang::Tr);
        assert_eq!(from_tag("tr"), Lang::Tr);
        assert_eq!(from_tag("en-US"), Lang::En);
        assert_eq!(from_tag("de-DE"), Lang::En);
        assert_eq!(from_tag("trk"), Lang::En);
    }
}
