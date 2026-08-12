//! Icône résidente (barre de menus macOS, zone de notification Windows,
//! StatusNotifier Linux) : l'application vit fenêtre cachée, le tray permet
//! de la rouvrir ou de quitter réellement, et porte le badge non-lus.
//!
//! Réveil sans rendu : les callbacks tray/menu poussent l'événement dans une
//! file statique puis réveillent egui via le `UiContext` partagé — le même
//! mécanisme que le réveil réseau. L'`update()` suivant dépile via `poll()`.

#[cfg(feature = "tray")]
use std::sync::Mutex;

#[cfg(all(feature = "tray", not(target_os = "linux")))]
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
#[cfg(all(feature = "tray", not(target_os = "linux")))]
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[cfg(feature = "tray")]
use crate::util::MutexExt;

/// Backend Linux : StatusNotifierItem sur D-Bus, sans GTK ni libappindicator.
#[cfg(all(feature = "tray", target_os = "linux"))]
mod sni;

/// Action utilisateur issue du tray, consommée par `AbcomApp::update`.
///
/// Le type reste défini sans la feature `tray` — les appelants s'en servent
/// dans un `match` que `poll()` ne peuplera simplement jamais.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub(crate) enum TrayAction {
    Open,
    Quit,
}

/// File d'événements bruts remplie par les callbacks (threads variés) et
/// drainée sur le thread UI.
#[cfg(feature = "tray")]
enum RawEvent {
    #[cfg(not(target_os = "linux"))]
    Menu(MenuId),
    /// Clic gauche sur l'icône (convention Windows : ouvrir).
    #[cfg(not(target_os = "linux"))]
    Click,
    /// Linux : le backend SNI résout lui-même clic et entrées de menu.
    #[cfg(target_os = "linux")]
    Action(TrayAction),
}

/// Événements en attente, avec l'instant où le système nous les a remis :
/// l'écart jusqu'au `poll()` mesure le délai de réveil de l'interface.
#[cfg(feature = "tray")]
static PENDING: Mutex<Vec<(RawEvent, std::time::Instant)>> = Mutex::new(Vec::new());

/// Contexte de réveil de l'UI, posé par `install_event_handlers`.
#[cfg(all(feature = "tray", target_os = "linux"))]
static WAKE: std::sync::OnceLock<crate::platform::notify::UiContext> = std::sync::OnceLock::new();

/// Retient de quoi réveiller l'UI : sous Linux les callbacks appartiennent au
/// backend SNI, il n'y a pas de handler global à poser.
#[cfg(all(feature = "tray", target_os = "linux"))]
pub(crate) fn install_event_handlers(ui_ctx: crate::platform::notify::UiContext) {
    let _ = WAKE.set(ui_ctx);
}

/// Met une action en file depuis le thread SNI, puis réveille l'interface.
#[cfg(all(feature = "tray", target_os = "linux"))]
fn queue(action: TrayAction) {
    PENDING
        .lock_safe()
        .push((RawEvent::Action(action), std::time::Instant::now()));
    match WAKE.get().and_then(|ctx| ctx.get()) {
        Some(ctx) => ctx.request_repaint(),
        // Menu utilisable avant que l'interface n'existe : l'événement
        // attendra la première frame.
        None => tracing::warn!("menu tray : interface pas encore prête, réveil impossible"),
    }
}

/// Installe les handlers globaux tray/menu : chaque événement est mis en
/// file puis réveille l'UI. À appeler une seule fois, avant la création.
#[cfg(all(feature = "tray", not(target_os = "linux")))]
pub(crate) fn install_event_handlers(ui_ctx: crate::platform::notify::UiContext) {
    let wake = ui_ctx.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        PENDING
            .lock_safe()
            .push((RawEvent::Menu(event.id), std::time::Instant::now()));
        match wake.get() {
            Some(ctx) => ctx.request_repaint(),
            // Menu utilisable avant que l'interface n'existe : l'événement
            // attendra la première frame.
            None => tracing::warn!("menu tray : interface pas encore prête, réveil impossible"),
        }
        wake_native_event_loop();
    }));
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        } = event
        {
            PENDING
                .lock_safe()
                .push((RawEvent::Click, std::time::Instant::now()));
        }
        if let Some(ctx) = ui_ctx.get() {
            ctx.request_repaint();
        }
        wake_native_event_loop();
    }));
}

/// Second réveil, au niveau du système : `request_repaint` seul ne suffit pas
/// dans tous les états de la fenêtre (voir `win::wake_event_loop`). Sans effet
/// hors Windows, où le problème n'a pas été observé.
#[cfg(all(feature = "tray", not(target_os = "linux")))]
fn wake_native_event_loop() {
    #[cfg(windows)]
    win::wake_event_loop();
}

#[cfg(feature = "tray")]
pub(crate) struct Tray {
    // Conservée en vie : la dropper retire l'icône du système. Sous Linux
    // l'icône appartient au thread SNI, qui la garde en vie lui-même.
    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)]
    icon: TrayIcon,
    #[cfg(not(target_os = "linux"))]
    normal: Icon,
    #[cfg(not(target_os = "linux"))]
    badge: Icon,
    #[cfg(not(target_os = "linux"))]
    open_id: MenuId,
    #[cfg(not(target_os = "linux"))]
    quit_id: MenuId,
    /// Linux : l'état du badge est poussé au thread SNI, seul à toucher l'icône.
    #[cfg(target_os = "linux")]
    unread_tx: tokio::sync::mpsc::UnboundedSender<bool>,
    badge_shown: bool,
}

#[cfg(feature = "tray")]
impl Tray {
    /// Crée l'icône résidente. macOS : doit être appelé sur le thread
    /// principal, event loop démarrée (premier `update()`). Renvoie `None`
    /// si le système n'offre pas de tray (l'appelant retombe alors sur le
    /// comportement « la croix quitte »).
    #[cfg(target_os = "linux")]
    pub(crate) fn new(open_label: &str, quit_label: &str) -> Option<Self> {
        let unread_tx = sni::spawn(open_label.to_owned(), quit_label.to_owned())?;
        Some(Self {
            unread_tx,
            badge_shown: false,
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn new(open_label: &str, quit_label: &str) -> Option<Self> {
        let (normal, badge) = build_icons()?;

        let menu = Menu::new();
        let open_item = MenuItem::new(open_label, true, None);
        let quit_item = MenuItem::new(quit_label, true, None);
        menu.append(&open_item).ok()?;
        menu.append(&PredefinedMenuItem::separator()).ok()?;
        menu.append(&quit_item).ok()?;
        let open_id = open_item.id().clone();
        let quit_id = quit_item.id().clone();

        let builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Abcom")
            .with_icon(normal.clone());
        // macOS : le clic gauche ouvre le menu (convention barre de menus) ;
        // ailleurs, le clic gauche ouvre la fenêtre (géré via TrayIconEvent).
        #[cfg(target_os = "macos")]
        let builder = builder.with_menu_on_left_click(true);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_menu_on_left_click(false);

        let icon = builder.build().ok()?;
        Some(Self {
            icon,
            normal,
            badge,
            open_id,
            quit_id,
            badge_shown: false,
        })
    }

    /// Dépile les événements tray reçus depuis la dernière frame.
    pub(crate) fn poll(&self) -> Vec<TrayAction> {
        PENDING
            .lock()
            .unwrap()
            .drain(..)
            .filter_map(|(raw, queued_at)| {
                let action = match raw {
                    #[cfg(not(target_os = "linux"))]
                    RawEvent::Menu(id) if id == self.open_id => Some(TrayAction::Open),
                    #[cfg(not(target_os = "linux"))]
                    RawEvent::Menu(id) if id == self.quit_id => Some(TrayAction::Quit),
                    #[cfg(not(target_os = "linux"))]
                    RawEvent::Menu(_) => None,
                    #[cfg(not(target_os = "linux"))]
                    RawEvent::Click => Some(TrayAction::Open),
                    #[cfg(target_os = "linux")]
                    RawEvent::Action(action) => Some(action),
                };
                // Doit être de l'ordre de la milliseconde : au-delà, c'est que
                // le réveil de l'interface n'a pas eu lieu et que l'action a
                // attendu un autre événement pour être vue.
                if let Some(action) = action {
                    tracing::info!(
                        ?action,
                        attente_ms = queued_at.elapsed().as_millis(),
                        "action tray dépilée"
                    );
                }
                action
            })
            .collect()
    }

    /// Affiche/retire la pastille non-lus sur l'icône.
    ///
    /// Sous Linux, l'icône appartient au thread SNI : on lui pousse l'état,
    /// il l'applique à la réception.
    #[cfg(target_os = "linux")]
    pub(crate) fn set_unread(&mut self, unread: bool) {
        if unread == self.badge_shown {
            return;
        }
        self.badge_shown = unread;
        let _ = self.unread_tx.send(unread);
    }

    /// Affiche/retire la pastille non-lus sur l'icône.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn set_unread(&mut self, unread: bool) {
        if unread == self.badge_shown {
            return;
        }
        self.badge_shown = unread;
        let icon = if unread {
            self.badge.clone()
        } else {
            self.normal.clone()
        };
        let _ = self.icon.set_icon(Some(icon));
    }
}

// ── Sans la feature `tray` ───────────────────────────────────────────────────
//
// Ces bouchons gardent la même API pour que les appelants restent identiques ;
// l'application se comporte alors comme sur un bureau sans zone de
// notification — fermer la fenêtre quitte réellement.

/// Sans tray, aucun handler global à poser.
#[cfg(not(feature = "tray"))]
pub(crate) fn install_event_handlers(_ui_ctx: crate::platform::notify::UiContext) {}

#[cfg(not(feature = "tray"))]
pub(crate) struct Tray;

#[cfg(not(feature = "tray"))]
impl Tray {
    /// `None` : l'appelant retombe sur le comportement « pas de tray ».
    pub(crate) fn new(_open_label: &str, _quit_label: &str) -> Option<Self> {
        None
    }

    pub(crate) fn poll(&self) -> Vec<TrayAction> {
        Vec::new()
    }

    pub(crate) fn set_unread(&mut self, _unread: bool) {}
}

/// Capte une fois le HWND natif de la fenêtre depuis `eframe::Frame`, pour
/// pouvoir la replier/restaurer au niveau OS (Windows uniquement).
#[cfg(windows)]
pub(crate) fn capture_window_handle(frame: &eframe::Frame) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if win::has_handle() {
        return;
    }
    if let Ok(handle) = frame.window_handle() {
        if let RawWindowHandle::Win32(win32) = handle.as_raw() {
            win::set_handle(win32.hwnd.get());
        }
    }
}

/// Repli/restauration natifs de la fenêtre sous Windows. On ne la cache PAS
/// (`SW_HIDE` couperait les `WM_PAINT` et donc la boucle egui) : on la sort de
/// l'écran et de la barre des tâches (style « outil ») en la gardant visible,
/// pour que le tray et les notifications continuent de fonctionner comme sur
/// macOS.
#[cfg(windows)]
pub(crate) mod win {
    use std::sync::atomic::{AtomicI32, AtomicIsize, Ordering};

    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Gdi::{RedrawWindow, RDW_INTERNALPAINT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, GetWindowRect, PostMessageW, SetForegroundWindow, SetWindowLongPtrW,
        SetWindowPos, ShowWindow, GWL_EXSTYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOSIZE,
        SWP_NOZORDER, SW_HIDE, SW_SHOW, SW_SHOWNA, WM_NULL, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    /// Position hors du bureau visible où l'on parque la fenêtre repliée.
    const OFFSCREEN: i32 = -32000;

    static HANDLE: AtomicIsize = AtomicIsize::new(0);
    static SAVED_X: AtomicI32 = AtomicI32::new(0);
    static SAVED_Y: AtomicI32 = AtomicI32::new(0);

    pub(crate) fn set_handle(hwnd: isize) {
        HANDLE.store(hwnd, Ordering::Relaxed);
    }

    pub(crate) fn has_handle() -> bool {
        HANDLE.load(Ordering::Relaxed) != 0
    }

    fn handle() -> Option<HWND> {
        let raw = HANDLE.load(Ordering::Relaxed);
        (raw != 0).then_some(raw as HWND)
    }

    /// Réveille la boucle de messages de la fenêtre, sans rien redessiner.
    ///
    /// `Context::request_repaint` ne suffit pas dans tous les états : egui
    /// n'appelle le rappel de réveil que si le délai demandé est **plus court**
    /// que celui déjà en attente, et remet ce délai à l'infini à la fin de
    /// chaque passe. Or, fenêtre minimisée ou masquée, eframe ne fait plus
    /// aucune passe complète : le délai n'est jamais remis à zéro, toute
    /// demande suivante est donc jugée redondante et ignorée. L'application
    /// dort alors jusqu'au prochain événement système — 3 min 25 mesurées
    /// entre un clic sur « Quitter » et sa prise en compte.
    ///
    /// Poster un message à la fenêtre contourne entièrement cette
    /// comptabilité : winit sort de son attente et eframe refait une passe,
    /// qui dépile la file du tray.
    pub(crate) fn wake_event_loop() {
        let Some(hwnd) = handle() else {
            return;
        };
        // SAFETY : `hwnd` est la fenêtre vivante de l'application.
        //
        // `RDW_INTERNALPAINT` met un `WM_PAINT` en file sans invalider la
        // moindre région : winit le traduit en `RedrawRequested`, dont eframe
        // fait toujours une passe — c'est ce qui dépile le tray. Le `WM_NULL`
        // qui suit ne sert qu'à sortir la boucle de son attente dans les états
        // où Windows ne délivre pas de `WM_PAINT` (fenêtre minimisée).
        unsafe {
            RedrawWindow(
                hwnd,
                std::ptr::null(),
                std::ptr::null_mut(),
                RDW_INTERNALPAINT,
            );
            PostMessageW(hwnd, WM_NULL, 0, 0);
        }
    }

    /// Replie la fenêtre : mémorise sa position, la passe en style « outil »
    /// (hors barre des tâches / Alt+Tab) et la déplace hors écran. Elle reste
    /// visible au sens de Windows, donc egui continue de tourner.
    pub(crate) fn hide_offscreen() {
        let Some(hwnd) = handle() else {
            return;
        };
        unsafe {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if GetWindowRect(hwnd, &mut rect) != 0 && rect.left > OFFSCREEN {
                SAVED_X.store(rect.left, Ordering::Relaxed);
                SAVED_Y.store(rect.top, Ordering::Relaxed);
            }

            // Le passage en « outil » ne met à jour la barre des tâches
            // qu'après un cycle hide/show ; on le fait pendant que la fenêtre
            // part hors écran, puis on la ré-affiche SANS l'activer (SW_SHOWNA).
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let ex = (ex & !(WS_EX_APPWINDOW as isize)) | WS_EX_TOOLWINDOW as isize;
            ShowWindow(hwnd, SW_HIDE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                OFFSCREEN,
                OFFSCREEN,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
            ShowWindow(hwnd, SW_SHOWNA);
        }
    }

    /// Restaure la fenêtre : retire le style « outil », la ramène à sa position
    /// mémorisée, l'affiche et lui donne le premier plan.
    pub(crate) fn restore_onscreen() {
        let Some(hwnd) = handle() else {
            return;
        };
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let ex = (ex & !(WS_EX_TOOLWINDOW as isize)) | WS_EX_APPWINDOW as isize;
            ShowWindow(hwnd, SW_HIDE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                SAVED_X.load(Ordering::Relaxed),
                SAVED_Y.load(Ordering::Relaxed),
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }
    }
}

/// Taille de l'icône tray (points logiques ; les OS remettent à l'échelle).
#[cfg(feature = "tray")]
const TRAY_PX: u32 = 32;

/// Les deux icônes en RGBA (normale, avec pastille rouge), depuis l'icône de
/// l'application embarquée.
#[cfg(feature = "tray")]
fn icon_rgba() -> Option<(Vec<u8>, Vec<u8>)> {
    let data = include_bytes!("../../assets/app_icon.png");
    let img = image::load_from_memory(data).ok()?;
    let small = img
        .resize_exact(TRAY_PX, TRAY_PX, image::imageops::FilterType::Lanczos3)
        .to_rgba8();

    // Pastille rouge en haut à droite (badge non-lus).
    let mut badged = small.clone();
    let (cx, cy, r) = (TRAY_PX as i32 - 8, 8i32, 7i32);
    for y in 0..TRAY_PX as i32 {
        for x in 0..TRAY_PX as i32 {
            let (dx, dy) = (x - cx, y - cy);
            if dx * dx + dy * dy <= r * r {
                badged.put_pixel(x as u32, y as u32, image::Rgba([220, 40, 60, 255]));
            }
        }
    }
    Some((small.into_raw(), badged.into_raw()))
}

/// Construit les deux icônes au format attendu par `tray-icon`.
#[cfg(all(feature = "tray", not(target_os = "linux")))]
fn build_icons() -> Option<(Icon, Icon)> {
    let (normal, badged) = icon_rgba()?;
    Some((
        Icon::from_rgba(normal, TRAY_PX, TRAY_PX).ok()?,
        Icon::from_rgba(badged, TRAY_PX, TRAY_PX).ok()?,
    ))
}
