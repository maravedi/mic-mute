use crate::config::AppVars;
use crate::icons::{rasterize_svg, tray_icon_color};
use crate::settings::{OverlayPosition, ShortcutConfig};
use anyhow::{Context, Result};
use log::trace;
use muda::{accelerator::Accelerator, CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem};
use std::fmt;
use tao::window::Theme;
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

const MUTE_TEXT: &str = "Mute";
const UNMUTE_TEXT: &str = "Unmute";

pub fn get_mute_menu_text(muted: bool) -> &'static str {
    if muted {
        UNMUTE_TEXT
    } else {
        MUTE_TEXT
    }
}

fn get_image(muted: bool, theme: Theme) -> Result<(Vec<u8>, u32, u32)> {
    const MIC_ON: &[u8] = include_bytes!("../assets/mic.svg");
    const MIC_OFF: &[u8] = include_bytes!("../assets/mic-off.svg");
    let svg = if muted { MIC_OFF } else { MIC_ON };
    rasterize_svg(svg, &tray_icon_color(muted, theme))
}

fn get_icon(muted: bool, theme: Theme) -> Result<Icon> {
    trace!("Fetching icons");
    let (icon_rgba, icon_width, icon_height) = get_image(muted, theme)?;
    let icon =
        Icon::from_rgba(icon_rgba, icon_width, icon_height).context("Failed to open icon")?;
    Ok(icon)
}

fn accelerator_from_config(config: &ShortcutConfig) -> Accelerator {
    let parts = config
        .modifiers
        .iter()
        .map(|modifier| match modifier.as_str() {
            "meta" => "cmd".to_string(),
            other => other.to_string(),
        });
    let accelerator = parts
        .chain(std::iter::once(config.key.clone()))
        .collect::<Vec<_>>()
        .join("+");

    accelerator
        .parse::<Accelerator>()
        .unwrap_or_else(|_| "A".parse().unwrap())
}

unsafe impl Send for Tray {}
unsafe impl Sync for Tray {}

impl fmt::Debug for Tray {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TrayIcon ID: {:?}", self.systray.id())
    }
}

pub struct Tray {
    pub systray: TrayIcon,
    pub toggle_mute: MenuItem,
    pub launch_at_login: CheckMenuItem,
    pub show_in_dock: CheckMenuItem,
    pub show_popup: CheckMenuItem,
    pub diagnostic_logging: CheckMenuItem,
    pub open_diagnostic_log: MenuItem,
    pub open_settings: MenuItem,
    pub overlay_top_left: CheckMenuItem,
    pub overlay_top_center: CheckMenuItem,
    pub overlay_top_right: CheckMenuItem,
    pub overlay_bottom_left: CheckMenuItem,
    pub overlay_bottom_center: CheckMenuItem,
    pub overlay_bottom_right: CheckMenuItem,
    pub about: MenuItem,
    pub quit: MenuItem,
}

impl Tray {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        muted: bool,
        theme: Theme,
        app_vars: AppVars,
        login_enabled: bool,
        dock_visible: bool,
        popup_visible: bool,
        diagnostic_logging_enabled: bool,
        overlay_position: OverlayPosition,
        mic_shortcut: &ShortcutConfig,
    ) -> Result<Self> {
        trace!("Creating tray icon");
        let icon = get_icon(muted, theme)?;
        let tray_menu = Menu::new();
        let toggle_mute = MenuItem::new(
            get_mute_menu_text(muted),
            true,
            Some(accelerator_from_config(mic_shortcut)),
        );
        let launch_at_login = CheckMenuItem::new("Launch at Login", true, login_enabled, None);
        let show_in_dock = CheckMenuItem::new("Show in Dock", true, dock_visible, None);
        let show_popup = CheckMenuItem::new("Show Popup", true, popup_visible, None);
        let diagnostic_logging =
            CheckMenuItem::new("Diagnostic Logging", true, diagnostic_logging_enabled, None);
        let open_diagnostic_log = MenuItem::new("Open Diagnostic Log…", true, None);
        let open_settings = MenuItem::new("Open Settings…", true, None);
        let overlay_position_submenu = muda::Submenu::new("Overlay Position", true);
        let overlay_top_left = CheckMenuItem::new(
            "Top Left",
            true,
            overlay_position == OverlayPosition::TopLeft,
            None,
        );
        let overlay_top_center = CheckMenuItem::new(
            "Top Center",
            true,
            overlay_position == OverlayPosition::TopCenter,
            None,
        );
        let overlay_top_right = CheckMenuItem::new(
            "Top Right",
            true,
            overlay_position == OverlayPosition::TopRight,
            None,
        );
        let overlay_bottom_left = CheckMenuItem::new(
            "Bottom Left",
            true,
            overlay_position == OverlayPosition::BottomLeft,
            None,
        );
        let overlay_bottom_center = CheckMenuItem::new(
            "Bottom Center",
            true,
            overlay_position == OverlayPosition::BottomCenter,
            None,
        );
        let overlay_bottom_right = CheckMenuItem::new(
            "Bottom Right",
            true,
            overlay_position == OverlayPosition::BottomRight,
            None,
        );
        overlay_position_submenu
            .append_items(&[
                &overlay_top_left,
                &overlay_top_center,
                &overlay_top_right,
                &PredefinedMenuItem::separator(),
                &overlay_bottom_left,
                &overlay_bottom_center,
                &overlay_bottom_right,
            ])
            .context("Failed to append overlay position menu items")?;
        let about = MenuItem::new("About", true, None);
        let quit = MenuItem::new("Exit", true, None);

        tray_menu
            .append_items(&[
                &toggle_mute,
                &PredefinedMenuItem::separator(),
                &launch_at_login,
                &show_in_dock,
                &show_popup,
                &overlay_position_submenu,
                &diagnostic_logging,
                &open_diagnostic_log,
                &open_settings,
                &about,
                &PredefinedMenuItem::separator(),
                &quit,
            ])
            .context("Failed to append menu items")?;

        let systray = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip(format!("{} service is running", app_vars.name))
            .with_icon(icon)
            .with_menu_on_left_click(true)
            .build()
            .context("Failed to create tray icon")?;

        trace!("Tray item created");
        let tray = Self {
            systray,
            toggle_mute,
            launch_at_login,
            show_in_dock,
            show_popup,
            diagnostic_logging,
            open_diagnostic_log,
            open_settings,
            overlay_top_left,
            overlay_top_center,
            overlay_top_right,
            overlay_bottom_left,
            overlay_bottom_center,
            overlay_bottom_right,
            about,
            quit,
        };
        Ok(tray)
    }

    pub fn update(&mut self, muted: bool, theme: Theme) -> Result<()> {
        log::info!(
            "Updating tray: muted={}, menu_item={}",
            muted,
            get_mute_menu_text(muted)
        );
        self.update_icon(muted, theme)?;
        self.update_menu(muted)?;
        Ok(())
    }

    fn update_icon(&mut self, muted: bool, theme: Theme) -> Result<()> {
        let icon = get_icon(muted, theme)?;
        self.systray.set_icon(Some(icon))?;
        trace!("Updated tray icon");
        Ok(())
    }

    fn update_menu(&mut self, muted: bool) -> Result<()> {
        self.toggle_mute.set_text(get_mute_menu_text(muted));
        trace!("Updated tray menu item");
        Ok(())
    }

    /// Update the displayed keyboard shortcuts after settings change.
    pub fn update_accelerators(&mut self, mic_shortcut: &ShortcutConfig) -> Result<()> {
        self.toggle_mute
            .set_accelerator(Some(accelerator_from_config(mic_shortcut)))
            .context("Failed to update mic accelerator")?;
        Ok(())
    }

    pub fn toggle_mute_id(&self) -> &MenuId {
        self.toggle_mute.id()
    }

    pub fn launch_at_login_id(&self) -> &MenuId {
        self.launch_at_login.id()
    }

    pub fn show_in_dock_id(&self) -> &MenuId {
        self.show_in_dock.id()
    }

    pub fn show_popup_id(&self) -> &MenuId {
        self.show_popup.id()
    }

    pub fn diagnostic_logging_id(&self) -> &MenuId {
        self.diagnostic_logging.id()
    }

    pub fn open_diagnostic_log_id(&self) -> &MenuId {
        self.open_diagnostic_log.id()
    }

    pub fn open_settings_id(&self) -> &MenuId {
        self.open_settings.id()
    }

    pub fn overlay_position_ids(&self) -> Vec<(MenuId, OverlayPosition)> {
        vec![
            (self.overlay_top_left.id().clone(), OverlayPosition::TopLeft),
            (
                self.overlay_top_center.id().clone(),
                OverlayPosition::TopCenter,
            ),
            (
                self.overlay_top_right.id().clone(),
                OverlayPosition::TopRight,
            ),
            (
                self.overlay_bottom_left.id().clone(),
                OverlayPosition::BottomLeft,
            ),
            (
                self.overlay_bottom_center.id().clone(),
                OverlayPosition::BottomCenter,
            ),
            (
                self.overlay_bottom_right.id().clone(),
                OverlayPosition::BottomRight,
            ),
        ]
    }

    pub fn set_overlay_position(&self, position: OverlayPosition) {
        self.overlay_top_left
            .set_checked(position == OverlayPosition::TopLeft);
        self.overlay_top_center
            .set_checked(position == OverlayPosition::TopCenter);
        self.overlay_top_right
            .set_checked(position == OverlayPosition::TopRight);
        self.overlay_bottom_left
            .set_checked(position == OverlayPosition::BottomLeft);
        self.overlay_bottom_center
            .set_checked(position == OverlayPosition::BottomCenter);
        self.overlay_bottom_right
            .set_checked(position == OverlayPosition::BottomRight);
    }

    pub fn about_id(&self) -> &MenuId {
        self.about.id()
    }

    pub fn quit_id(&self) -> &MenuId {
        self.quit.id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_mute_menu_text_muted() {
        assert_eq!(get_mute_menu_text(true), "Unmute");
    }

    #[test]
    fn test_get_mute_menu_text_unmuted() {
        assert_eq!(get_mute_menu_text(false), "Mute");
    }
}
