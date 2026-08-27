# Settings

Mic Mute stores its settings at:

```text
~/Library/Application Support/mic-mute/settings.json
```

The file is JSON. The complete default configuration is:

```json
{
  "mic_shortcut": {
    "modifiers": ["shift", "meta"],
    "key": "A"
  },
  "show_in_dock": false,
  "launch_at_login": false,
  "show_popup": true,
  "diagnostic_logging": false,
  "overlay_position": "top_center"
}
```

## Options

### `mic_shortcut`

Controls the global microphone mute shortcut.

- `modifiers`: zero or more of `shift`, `meta`/`cmd`/`command`, `ctrl`/`control`, and `alt`/`option`.
- `key`: a key such as `A`, `M`, or `F13`.

The default is `Cmd` + `Shift` + `A`.

### `show_in_dock`

Controls whether Mic Mute appears in the macOS Dock.

- `false` (default): run as a menu bar accessory.
- `true`: show an application icon in the Dock.

The tray menu can also toggle this setting.

### `diagnostic_logging`

Controls detailed diagnostic logging.

- `false` (default): do not write new diagnostic entries.
- `true`: write diagnostics to `~/Library/Logs/mic-mute/mic-mute.log`.

Choose **Open Diagnostic Log…** from the tray menu to open this file. The
tray menu also includes **Open Settings…** to open this JSON file directly.

### `overlay_position`

Controls the popup's anchor within the screen currently under the cursor. It
does not lock the popup to a particular monitor.

- `top_left`, `top_center` (default), or `top_right`
- `bottom_left`, `bottom_center`, or `bottom_right`

The tray menu's **Overlay Position** submenu can also change this setting.

### `launch_at_login`

Controls whether Mic Mute opens when the user logs in.

- `false` (default)
- `true`

The tray menu can also toggle this setting.

### `show_popup`

Controls the small on-screen mute-status popup.

- `true` (default): show the popup when the microphone is muted.
- `false`: keep the popup hidden while retaining the tray indicator.

The tray menu can also toggle this setting.

## Editing settings

Mic Mute checks the file every two seconds and reloads it when its modification time changes. Valid changes apply without a restart. The tray menu writes its setting changes to this file.

Fields omitted from the settings file use the documented defaults, including `show_popup: true`. Loading settings does not rewrite the file. The file is created or updated when settings are explicitly saved, such as through a tray-menu setting change. Normal saves serialize the documented settings fields.
