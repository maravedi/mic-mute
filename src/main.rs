mod about;
mod camera;
mod config;
mod event_loop;
mod icons;
mod launch_at_login;
mod mic;
mod popup;
mod popup_content;
mod settings;
mod shortcuts;
mod tray;
mod ui;
mod utils;
// TODO: Use better Apple logging support? https://lib.rs/crates/oslog

#[macro_use]
extern crate objc;

use crate::camera::CameraController;
use crate::config::AppVars;
use crate::event_loop::{restore_microphone_on_exit, start};
use crate::mic::MicController;
use crate::settings::Settings;
use crate::ui::UI;
use crate::utils::arc_lock;
use anyhow::{Context, Result};
use env_logger::{Builder, Target};
use log::{info, trace};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_signal(_: libc::c_int) {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn diagnostic_log_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join("Library/Logs/mic-mute/mic-mute.log"))
}

pub fn open_diagnostic_log() -> Result<()> {
    let path = diagnostic_log_path().context("Diagnostic log directory unavailable")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new().create(true).append(true).open(&path)?;
    Command::new("open").arg("-t").arg(path).spawn()?;
    Ok(())
}

fn init_logging(enabled: bool) {
    // Finder inherits the desktop session's RUST_LOG value. Always retain full
    // diagnostics in the app's private log file so UI/mute state can be
    // investigated even when that session-level value is restrictive.
    let mut builder = Builder::new();
    builder.filter_level(log::LevelFilter::Trace);
    if let Some(path) = diagnostic_log_path() {
        if let Some(parent) = path.parent() {
            if fs::create_dir_all(parent).is_ok() {
                if let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) {
                    builder.target(Target::Pipe(Box::new(file)));
                }
            }
        }
    }
    builder.init();
    set_diagnostic_logging(enabled);
}

pub fn set_diagnostic_logging(enabled: bool) {
    if enabled {
        log::set_max_level(log::LevelFilter::Trace);
        info!("Diagnostic logging enabled");
    } else {
        info!("Diagnostic logging disabled");
        log::set_max_level(log::LevelFilter::Off);
    }
}

fn main() {
    let mut settings = Settings::load();
    init_logging(settings.diagnostic_logging);
    info!("Starting app");
    if let Some(path) = diagnostic_log_path() {
        info!("Diagnostic log: {}", path.display());
    }

    // On first run (or after upgrading from a version without launch_at_login in
    // settings), adopt the existing plist state so we don't silently disable it.
    let plist_enabled = launch_at_login::is_enabled();
    if plist_enabled != settings.launch_at_login {
        settings.launch_at_login = plist_enabled;
        let _ = settings.save();
    }

    let app_vars = AppVars::new();

    let controller = MicController::new().unwrap();
    let mic_muted = controller.muted;
    let controller = arc_lock(controller);
    trace!("Mic controller initialized {:?}", controller);

    // Register SIGTERM/SIGINT handlers. The signal handler only sets a flag;
    // a background thread performs microphone cleanup before exiting.
    unsafe {
        libc::signal(
            libc::SIGTERM,
            handle_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGINT,
            handle_signal as *const () as libc::sighandler_t,
        );
    }
    let shutdown_controller = controller.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(100));
        if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
            info!("Signal received — restoring microphone state before exit");
            restore_microphone_on_exit(&shutdown_controller);
            std::process::exit(0);
        }
    });

    let camera = CameraController::new().unwrap();
    let camera_muted = camera.muted;
    let camera = arc_lock(camera);
    trace!("Camera controller initialized, muted={}", camera_muted);

    let (ui, event_loop, event_ids) =
        UI::new(mic_muted, camera_muted, app_vars, &settings).unwrap();
    trace!("UI initialized");
    let ui = arc_lock(ui);
    let settings = arc_lock(settings);
    start(event_loop, event_ids, ui, controller, camera, settings);
}
