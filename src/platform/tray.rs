//! Icône résidente (barre de menus macOS, zone de notification Windows,
//! StatusNotifier Linux) : l'application vit fenêtre cachée, le tray permet
//! de la rouvrir ou de quitter réellement, et porte le badge non-lus.
//!
//! Réveil sans rendu : les callbacks tray/menu poussent l'événement dans une
//! file statique puis réveillent egui via le `UiContext` partagé — le même
//! mécanisme que le réveil réseau. L'`update()` suivant dépile via `poll()`.

#[cfg(feature = "tray")]
use std::sync::Mutex;

#[cfg(feature = "tray")]
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
#[cfg(feature = "tray")]
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[cfg(feature = "tray")]
use crate::util::MutexExt;

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
    Menu(MenuId),
    /// Clic gauche sur l'icône (convention Windows/Linux : ouvrir).
    Click,
}

#[cfg(feature = "tray")]
static PENDING: Mutex<Vec<RawEvent>> = Mutex::new(Vec::new());

/// Installe les handlers globaux tray/menu : chaque événement est mis en
/// file puis réveille l'UI. À appeler une seule fois, avant la création.
#[cfg(feature = "tray")]
pub(crate) fn install_event_handlers(ui_ctx: crate::platform::notify::UiContext) {
    let wake = ui_ctx.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        PENDING.lock_safe().push(RawEvent::Menu(event.id));
        if let Some(ctx) = wake.get() {
            ctx.request_repaint();
        }
    }));
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        } = event
        {
            PENDING.lock_safe().push(RawEvent::Click);
        }
        if let Some(ctx) = ui_ctx.get() {
            ctx.request_repaint();
        }
    }));
}

#[cfg(feature = "tray")]
pub(crate) struct Tray {
    // Conservée en vie : la dropper retire l'icône du système. Sous Linux
    // l'icône appartient au thread GTK, qui la garde en vie lui-même.
    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)]
    icon: TrayIcon,
    #[cfg(not(target_os = "linux"))]
    normal: Icon,
    #[cfg(not(target_os = "linux"))]
    badge: Icon,
    open_id: MenuId,
    quit_id: MenuId,
    badge_shown: bool,
}

/// Badge non-lus voulu par l'UI. Sous Linux, l'icône ne peut être touchée que
/// depuis le thread GTK : l'UI publie ici son intention, le thread l'applique.
#[cfg(all(feature = "tray", target_os = "linux"))]
static UNREAD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Monte la boucle GTK et l'icône sur un thread dédié, et renvoie les
/// identifiants des deux entrées de menu.
///
/// `tray-icon` impose sous Linux que l'icône soit créée sur un thread faisant
/// tourner une boucle GTK ; winit n'en fournit pas. Sans cela, `Menu::new()`
/// panique (« GTK has not been initialized ») avant même que le repli prévu
/// par l'appelant ne puisse jouer.
#[cfg(all(feature = "tray", target_os = "linux"))]
fn spawn_gtk_thread(open_label: String, quit_label: String) -> Option<(MenuId, MenuId)> {
    use std::sync::atomic::Ordering;

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("abcom-tray-gtk".into())
        .spawn(move || {
            // Chaque étape peut échouer sur un poste sans zone de
            // notification : on prévient l'appelant plutôt que de paniquer.
            let build = || -> Option<((MenuId, MenuId), TrayIcon, Icon, Icon)> {
                gtk::init().ok()?;
                let (normal, badge) = build_icons()?;
                let menu = Menu::new();
                let open_item = MenuItem::new(&open_label, true, None);
                let quit_item = MenuItem::new(&quit_label, true, None);
                menu.append(&open_item).ok()?;
                menu.append(&PredefinedMenuItem::separator()).ok()?;
                menu.append(&quit_item).ok()?;
                let ids = (open_item.id().clone(), quit_item.id().clone());
                let icon = TrayIconBuilder::new()
                    .with_menu(Box::new(menu))
                    .with_tooltip("Abcom")
                    .with_icon(normal.clone())
                    .with_menu_on_left_click(false)
                    .build()
                    .ok()?;
                Some((ids, icon, normal, badge))
            };

            let Some((ids, icon, normal, badge)) = build() else {
                let _ = tx.send(None);
                return;
            };
            let _ = tx.send(Some(ids));

            // L'UI ne peut pas toucher l'icône depuis son thread : on relit son
            // intention ici. Au repos le drapeau ne bouge pas, ce réveil ne
            // coûte donc qu'une comparaison cinq fois par seconde.
            let mut applied = false;
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
                let wanted = UNREAD.load(Ordering::Relaxed);
                if wanted != applied {
                    applied = wanted;
                    let next = if wanted {
                        badge.clone()
                    } else {
                        normal.clone()
                    };
                    let _ = icon.set_icon(Some(next));
                }
                gtk::glib::ControlFlow::Continue
            });

            gtk::main();
        })
        .ok()?;

    // Le thread répond dès que l'icône est posée ; au-delà, on considère que
    // le bureau n'offre pas de zone de notification.
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .ok()
        .flatten()
}

#[cfg(feature = "tray")]
impl Tray {
    /// Crée l'icône résidente. macOS : doit être appelé sur le thread
    /// principal, event loop démarrée (premier `update()`). Renvoie `None`
    /// si le système n'offre pas de tray (l'appelant retombe alors sur le
    /// comportement « la croix quitte »).
    #[cfg(target_os = "linux")]
    pub(crate) fn new(open_label: &str, quit_label: &str) -> Option<Self> {
        let (open_id, quit_id) = spawn_gtk_thread(open_label.to_owned(), quit_label.to_owned())?;
        Some(Self {
            open_id,
            quit_id,
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
            .filter_map(|raw| match raw {
                RawEvent::Menu(id) if id == self.open_id => Some(TrayAction::Open),
                RawEvent::Menu(id) if id == self.quit_id => Some(TrayAction::Quit),
                RawEvent::Menu(_) => None,
                RawEvent::Click => Some(TrayAction::Open),
            })
            .collect()
    }

    /// Affiche/retire la pastille non-lus sur l'icône.
    ///
    /// Sous Linux, l'icône appartient au thread GTK : on n'y publie qu'un
    /// drapeau, appliqué par ce thread à son prochain réveil.
    #[cfg(target_os = "linux")]
    pub(crate) fn set_unread(&mut self, unread: bool) {
        if unread == self.badge_shown {
            return;
        }
        self.badge_shown = unread;
        UNREAD.store(unread, std::sync::atomic::Ordering::Relaxed);
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
// `tray-icon` tire GTK3 et libappindicator sur Linux : sans en-têtes de
// développement, la compilation échoue avant même d'atteindre notre code.
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
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, GetWindowRect, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
        ShowWindow, GWL_EXSTYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
        SW_HIDE, SW_SHOW, SW_SHOWNA, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
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

/// Construit les deux icônes (normale, avec pastille rouge) depuis l'icône
/// de l'application embarquée.
#[cfg(feature = "tray")]
fn build_icons() -> Option<(Icon, Icon)> {
    let data = include_bytes!("../../assets/app_icon.png");
    let img = image::load_from_memory(data).ok()?;
    let small = img
        .resize_exact(TRAY_PX, TRAY_PX, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let normal = Icon::from_rgba(small.as_raw().clone(), TRAY_PX, TRAY_PX).ok()?;

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
    let badge = Icon::from_rgba(badged.into_raw(), TRAY_PX, TRAY_PX).ok()?;
    Some((normal, badge))
}
