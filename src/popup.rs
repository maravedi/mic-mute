use crate::event_loop::EventLoopMessage;
use crate::popup_content::PopupContent;
use crate::settings::OverlayPosition;
use crate::utils::get_cursor_pos;
use anyhow::{Context, Result};
use cocoa::{
    appkit::{NSView, NSWindow},
    base::{id, NO, YES},
};
use log::trace;
use tao::{
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    monitor::MonitorHandle,
    platform::macos::{WindowBuilderExtMacOS, WindowExtMacOS},
    window::{Theme, Window, WindowBuilder},
};

const MUTED_TITLE: &str = "Muted";
const UNMUTED_TITLE: &str = "Unmuted";

pub type WindowSize<T = f64> = LogicalSize<T>;

fn get_mute_title_text(muted: bool) -> &'static str {
    if muted {
        MUTED_TITLE
    } else {
        UNMUTED_TITLE
    }
}

fn should_show_popup(enabled: bool, muted: bool) -> bool {
    enabled && muted
}

fn monitor_contains_physical_position(
    position: PhysicalPosition<f64>,
    monitor_position: PhysicalPosition<f64>,
    monitor_size: PhysicalSize<f64>,
) -> bool {
    position.x >= monitor_position.x
        && position.x < monitor_position.x + monitor_size.width
        && position.y >= monitor_position.y
        && position.y < monitor_position.y + monitor_size.height
}

fn setup_window(window: id) {
    unsafe {
        window.setHasShadow_(true);
        let _: () = msg_send![window, setOpaque: NO];
        let clear: id = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![window, setBackgroundColor: clear];
    };
}

pub struct Popup {
    window: Window,
    content: PopupContent,
    current_monitor: Option<MonitorHandle>,
    enabled: bool,
    position: OverlayPosition,
}

impl Popup {
    pub fn new(
        event_loop: &EventLoopMessage,
        mic_muted: bool,
        enabled: bool,
        position: OverlayPosition,
    ) -> Result<Self> {
        let camera_muted = false;
        let initial_monitor = Popup::get_initial_monitor(event_loop);
        let size = Popup::get_size();
        let scale = initial_monitor
            .as_ref()
            .map_or(1.0, MonitorHandle::scale_factor);
        let mut builder = WindowBuilder::new()
            .with_title(get_mute_title_text(mic_muted))
            .with_titlebar_hidden(true)
            .with_movable_by_window_background(true)
            .with_always_on_top(true)
            .with_closable(false)
            .with_content_protection(false)
            .with_transparent(true)
            .with_decorations(false)
            .with_inner_size(size)
            .with_maximized(false)
            .with_minimizable(false)
            .with_resizable(false)
            .with_visible_on_all_workspaces(true)
            .with_visible(false)
            .with_has_shadow(true);
        if let Some(monitor) = initial_monitor.as_ref() {
            builder = builder.with_position(Popup::get_position(monitor, size, position));
        }
        let window = builder
            .build(event_loop)
            .context("Failed to build window")?;
        window.set_visible(false);
        window.set_ignore_cursor_events(true)?;

        trace!("Window scale factor {}", scale);
        let content = PopupContent::new(mic_muted, camera_muted, size, window.theme())?;
        unsafe {
            let ns_view = window.ns_view() as id;
            let _: () = msg_send![ns_view, setWantsLayer: YES];
            let layer: id = msg_send![ns_view, layer];
            let background: id = msg_send![class!(NSColor),
                colorWithRed: 0.11_f64 green: 0.11_f64 blue: 0.12_f64 alpha: 0.94_f64];
            let background_color: *const std::os::raw::c_void = msg_send![background, CGColor];
            let _: () = msg_send![layer, setBackgroundColor: background_color];
            let _: () = msg_send![layer, setCornerRadius: 14.0_f64];
            let _: () = msg_send![layer, setMasksToBounds: YES];
            ns_view.addSubview_(content.view);
            let _: () = msg_send![content.view, release];
            let ns_window = window.ns_window() as id;
            setup_window(ns_window);
        };

        let popup = Self {
            window,
            content,
            current_monitor: initial_monitor,
            enabled,
            position,
        };
        Ok(popup)
    }

    fn get_size() -> WindowSize {
        LogicalSize::new(300., 56.)
    }

    pub fn get_theme(&self) -> Theme {
        self.window.theme()
    }

    pub fn update_with_camera(
        &mut self,
        mic_muted: bool,
        camera_muted: bool,
        active_device_name: Option<&str>,
    ) -> Result<&mut Self> {
        self.window.set_title(get_mute_title_text(mic_muted));
        self.update_placement()?;
        self.content.update(
            mic_muted,
            camera_muted,
            self.get_theme(),
            active_device_name,
        )?;
        if should_show_popup(self.enabled, mic_muted) {
            self.show_front();
        }
        Ok(self)
    }

    pub fn hide(&mut self) -> Result<&mut Self> {
        self.window.set_visible(false);
        Ok(self)
    }
    pub fn set_enabled(&mut self, enabled: bool) -> Result<&mut Self> {
        self.enabled = enabled;
        if !enabled {
            self.hide()?;
        }
        Ok(self)
    }

    pub fn set_position(&mut self, position: OverlayPosition) -> Result<&mut Self> {
        self.position = position;
        self.update_placement()
    }

    pub fn update_placement(&mut self) -> Result<&mut Self> {
        if let Some(monitor) = self.get_current_monitor()? {
            let monitor_changed = self.current_monitor.as_ref() != Some(&monitor);
            let was_visible = monitor_changed && self.window.is_visible();
            if was_visible {
                self.window.set_visible(false);
            }

            let size = Popup::get_size();
            self.window.set_inner_size(size);
            self.window
                .set_outer_position(Popup::get_position(&monitor, size, self.position));
            self.current_monitor = Some(monitor);

            if was_visible {
                self.show_front();
            }
        }
        Ok(self)
    }

    pub fn detect_cursor_monitor(&mut self) -> Result<&mut Self> {
        self.update_placement()
    }

    fn get_current_monitor(&self) -> Result<Option<MonitorHandle>> {
        // CoreGraphics and `Window::monitor_from_point` both use the same global
        // display coordinate space on macOS. Prefer this path over
        // `Window::cursor_position`, which converts through the primary display's
        // scale factor and can misclassify points near monitor boundaries.
        if let Some((x, y)) = get_cursor_pos() {
            if let Some(monitor) = self.window.monitor_from_point(x, y) {
                return Ok(Some(monitor));
            }
        }

        let position = self
            .window
            .cursor_position()
            .context("Failed to read cursor position")?;
        if let Some(monitor) = self.window.monitor_from_point(position.x, position.y) {
            return Ok(Some(monitor));
        }

        Ok(self.monitor_from_physical_position(position))
    }

    fn monitor_from_physical_position(
        &self,
        position: PhysicalPosition<f64>,
    ) -> Option<MonitorHandle> {
        self.window.available_monitors().find(|monitor| {
            monitor_contains_physical_position(
                position,
                monitor.position().cast::<f64>(),
                monitor.size().cast::<f64>(),
            )
        })
    }

    fn get_initial_monitor(event_loop: &EventLoopMessage) -> Option<MonitorHandle> {
        event_loop.primary_monitor()
    }

    fn show_front(&self) {
        if !self.enabled {
            return;
        }

        self.window.set_visible(true);
        unsafe {
            let ns_window = self.window.ns_window() as id;
            let _: () = msg_send![ns_window, orderFrontRegardless];
        }
    }

    fn get_position(
        monitor: &MonitorHandle,
        window_size: WindowSize,
        position: OverlayPosition,
    ) -> LogicalPosition<f64> {
        let scale = monitor.scale_factor();
        let monitor_position = monitor.position().to_logical::<f64>(scale);
        let monitor_size = monitor.size().to_logical::<f64>(scale);
        popup_position_for_bounds(monitor_position, monitor_size, window_size, position)
    }
}

/// Translates a user-facing overlay anchor to Tao's macOS window coordinate
/// space. Kept independent from `MonitorHandle` so every anchor can be
/// verified without a physical display.
fn popup_position_for_bounds(
    monitor_position: LogicalPosition<f64>,
    monitor_size: LogicalSize<f64>,
    window_size: WindowSize,
    position: OverlayPosition,
) -> LogicalPosition<f64> {
    const HORIZONTAL_INSET: f64 = 24.;
    let x = match position {
        OverlayPosition::TopLeft | OverlayPosition::BottomLeft => {
            monitor_position.x + HORIZONTAL_INSET
        }
        OverlayPosition::TopCenter | OverlayPosition::BottomCenter => {
            (monitor_position.x + (monitor_size.width / 2.)) - (window_size.width / 2.)
        }
        OverlayPosition::TopRight | OverlayPosition::BottomRight => {
            monitor_position.x + monitor_size.width - window_size.width - HORIZONTAL_INSET
        }
    };
    // Tao's macOS window coordinates are inverted vertically relative to
    // the user-facing labels: smaller Y appears toward the top of a screen.
    let y = match position {
        OverlayPosition::TopLeft | OverlayPosition::TopCenter | OverlayPosition::TopRight => {
            monitor_position.y + window_size.height
        }
        OverlayPosition::BottomLeft
        | OverlayPosition::BottomCenter
        | OverlayPosition::BottomRight => {
            (monitor_position.y + monitor_size.height) - (window_size.height * 2.)
        }
    };
    LogicalPosition::new(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_contains_fractional_positions_on_negative_edges() {
        let monitor_position = PhysicalPosition::new(-1920.0, 0.0);
        let monitor_size = PhysicalSize::new(1920.0, 1080.0);

        assert!(monitor_contains_physical_position(
            PhysicalPosition::new(-0.25, 100.0),
            monitor_position,
            monitor_size
        ));
    }

    #[test]
    fn monitor_contains_positions_until_exclusive_far_edges() {
        let monitor_position = PhysicalPosition::new(0.0, 0.0);
        let monitor_size = PhysicalSize::new(1440.0, 900.0);

        assert!(monitor_contains_physical_position(
            PhysicalPosition::new(1439.999, 899.999),
            monitor_position,
            monitor_size
        ));
        assert!(!monitor_contains_physical_position(
            PhysicalPosition::new(1440.0, 899.999),
            monitor_position,
            monitor_size
        ));
        assert!(!monitor_contains_physical_position(
            PhysicalPosition::new(1439.999, 900.0),
            monitor_position,
            monitor_size
        ));
    }
    #[test]
    fn popup_visibility_requires_enabled_setting_and_mute() {
        assert!(should_show_popup(true, true));
        assert!(!should_show_popup(false, true));
        assert!(!should_show_popup(true, false));
    }

    #[test]
    fn overlay_position_labels_match_the_visible_screen_anchors() {
        let monitor_position = LogicalPosition::new(0., 0.);
        let monitor_size = LogicalSize::new(1440., 900.);
        let popup_size = LogicalSize::new(300., 56.);
        let anchors = [
            (OverlayPosition::TopLeft, 24., 56.),
            (OverlayPosition::TopCenter, 570., 56.),
            (OverlayPosition::TopRight, 1116., 56.),
            (OverlayPosition::BottomLeft, 24., 788.),
            (OverlayPosition::BottomCenter, 570., 788.),
            (OverlayPosition::BottomRight, 1116., 788.),
        ];

        for (label, expected_x, expected_y) in anchors {
            let actual =
                popup_position_for_bounds(monitor_position, monitor_size, popup_size, label);
            assert_eq!(
                actual.x, expected_x,
                "wrong horizontal anchor for {label:?}"
            );
            assert_eq!(actual.y, expected_y, "wrong vertical anchor for {label:?}");
        }
    }
}
