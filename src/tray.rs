use log::{debug, info};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    Icon, TrayIconBuilder, TrayIconEvent,
};

use slint::ComponentHandle;

thread_local! {
    static TRAY_ICON: std::cell::RefCell<Option<tray_icon::TrayIcon>> = std::cell::RefCell::new(None);
}

const MENU_ID_SHOW: &str = "show";
const MENU_ID_START_RECORDING: &str = "start-recording";
const MENU_ID_STOP_RECORDING: &str = "stop-recording";
const MENU_ID_SETTINGS: &str = "settings";
const MENU_ID_EXIT: &str = "exit";

#[cfg(windows)]
fn shortcut_ico_bytes_for_theme(theme: &str) -> &'static [u8] {
    match theme {
        "white" => include_bytes!("../icons/taskbar/white.ico"),
        "warm_gray" => include_bytes!("../icons/taskbar/warm_gray.ico"),
        "soft_beige" => include_bytes!("../icons/taskbar/soft_beige.ico"),
        "blue_gray" => include_bytes!("../icons/taskbar/blue_gray.ico"),
        "warm_taupe" => include_bytes!("../icons/taskbar/warm_taupe.ico"),
        "soft_sage" => include_bytes!("../icons/taskbar/soft_sage.ico"),
        "dark_charcoal" => include_bytes!("../icons/taskbar/dark_charcoal.ico"),
        "dark_blue" => include_bytes!("../icons/taskbar/dark_blue.ico"),
        "dark_purple" => include_bytes!("../icons/taskbar/dark_purple.ico"),
        "dark_forest" => include_bytes!("../icons/taskbar/dark_forest.ico"),
        "dark_burgundy" => include_bytes!("../icons/taskbar/dark_burgundy.ico"),
        "obsidian" => include_bytes!("../icons/taskbar/obsidian.ico"),
        _ => include_bytes!("../icons/taskbar/white.ico"),
    }
}

fn tray_icon_bytes_for_theme(theme: &str) -> &'static [u8] {
    match theme {
        "white" => include_bytes!("../icons/tray/white.png"),
        "warm_gray" => include_bytes!("../icons/tray/warm_gray.png"),
        "soft_beige" => include_bytes!("../icons/tray/soft_beige.png"),
        "blue_gray" => include_bytes!("../icons/tray/blue_gray.png"),
        "warm_taupe" => include_bytes!("../icons/tray/warm_taupe.png"),
        "soft_sage" => include_bytes!("../icons/tray/soft_sage.png"),
        "dark_charcoal" => include_bytes!("../icons/tray/dark_charcoal.png"),
        "dark_blue" => include_bytes!("../icons/tray/dark_blue.png"),
        "dark_purple" => include_bytes!("../icons/tray/dark_purple.png"),
        "dark_forest" => include_bytes!("../icons/tray/dark_forest.png"),
        "dark_burgundy" => include_bytes!("../icons/tray/dark_burgundy.png"),
        "obsidian" => include_bytes!("../icons/tray/obsidian.png"),
        _ => include_bytes!("../icons/tray/white.png"),
    }
}

pub fn setup_tray(
    theme: &str,
    app_weak: &slint::Weak<crate::App>,
) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_tray_menu(app_weak)?;

    let icon_image = icon_from_png_bytes(tray_icon_bytes_for_theme(theme))
        .map_err(|e| format!("Failed to decode tray icon: {}", e))?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("QuillScribe - Voice to Text")
        .with_icon(icon_image)
        .build()
        .map_err(|e| format!("Failed to build tray icon: {}", e))?;

    let weak = app_weak.clone();
    std::thread::Builder::new()
        .name("tray-event-loop".into())
        .spawn(move || {
            let menu_channel = MenuEvent::receiver();
            let tray_channel = TrayIconEvent::receiver();
            loop {
                if let Ok(event) = menu_channel.try_recv() {
                    handle_menu_event(event.id().0.as_str(), &weak);
                }
                if let Ok(event) = tray_channel.try_recv() {
                    if matches!(event, TrayIconEvent::Click { .. }) {
                        show_app(&weak);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        })
        .expect("Failed to spawn tray event loop thread");

    TRAY_ICON.with(|t| {
        *t.borrow_mut() = Some(tray);
    });

    info!("System tray initialized with theme: {}", theme);
    Ok(())
}

fn build_tray_menu(
    _app_weak: &slint::Weak<crate::App>,
) -> Result<Menu, Box<dyn std::error::Error>> {
    let menu = Menu::new();

    let show_item = MenuItem::with_id(MenuId::new(MENU_ID_SHOW), "Show QuillScribe", true, None);
    let start_item = MenuItem::with_id(MenuId::new(MENU_ID_START_RECORDING), "Start Recording", true, None);
    let stop_item = MenuItem::with_id(MenuId::new(MENU_ID_STOP_RECORDING), "Stop Recording", false, None);
    let separator = PredefinedMenuItem::separator();
    let settings_item = MenuItem::with_id(MenuId::new(MENU_ID_SETTINGS), "Settings", true, None);
    let quit_item = MenuItem::with_id(MenuId::new(MENU_ID_EXIT), "Exit", true, None);

    menu.append(&show_item)?;
    menu.append(&start_item)?;
    menu.append(&stop_item)?;
    menu.append(&separator)?;
    // TODO: model submenu
    menu.append(&settings_item)?;
    menu.append(&separator)?;
    menu.append(&quit_item)?;

    Ok(menu)
}

fn with_app<F>(app_weak: &slint::Weak<crate::App>, f: F)
where
    F: FnOnce(crate::App) + Send + 'static,
{
    let weak = app_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(app) = weak.upgrade() {
            f(app);
        }
    });
}

fn show_app(app_weak: &slint::Weak<crate::App>) {
    with_app(app_weak, |app| {
        app.window().set_minimized(false);
        app.window().show().ok();
    });
}

fn handle_menu_event(
    id: &str,
    app_weak: &slint::Weak<crate::App>,
) {
    match id {
        MENU_ID_SHOW => show_app(app_weak),
        MENU_ID_START_RECORDING => with_app(app_weak, |app| {
            app.invoke_toggle_recording();
        }),
        MENU_ID_STOP_RECORDING => with_app(app_weak, |app| {
            if app.get_is_recording() {
                app.invoke_toggle_recording();
            }
        }),
        MENU_ID_SETTINGS => with_app(app_weak, |app| {
            app.set_active_panel("settings".into());
            app.window().show().ok();
        }),
        MENU_ID_EXIT => with_app(app_weak, |app| {
            crate::window::quit_app(&app);
        }),
        _ => {}
    }
}

pub fn cleanup_tray() {
    TRAY_ICON.with(|t| {
        *t.borrow_mut() = None;
    });
}

pub fn set_tray_theme(theme: &str) {
    TRAY_ICON.with(|t| {
        if let Some(ref tray) = *t.borrow() {
            if let Ok(icon) = icon_from_png_bytes(tray_icon_bytes_for_theme(theme)) {
                tray.set_icon(Some(icon)).ok();
                debug!("Tray icon updated for theme: {}", theme);
            }
        }
    });
}

fn icon_from_png_bytes(png_bytes: &[u8]) -> Result<Icon, String> {
    let (rgba, w, h) = crate::window::decode_png_to_rgba(png_bytes)?;
    Icon::from_rgba(rgba, w, h)
        .map_err(|e| format!("Icon from RGBA error: {}", e))
}
