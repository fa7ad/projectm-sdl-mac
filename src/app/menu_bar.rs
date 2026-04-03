use std::collections::HashMap;

use muda::{
    CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers},
};
use sdl3::audio::AudioDeviceID;

pub enum MenuCommand {
    SwitchAudioDevice(AudioDeviceID),
    NextInputDevice,
    NextPreset,
    PrevPreset,
    RandomPreset,
}

pub struct MenuBar {
    _menu: Menu,
    fps_item: MenuItem,
    device_items: Vec<CheckMenuItem>,
    // MenuId → AudioDeviceID for device checkitems
    device_id_map: HashMap<MenuId, AudioDeviceID>,
    // Device name → index into device_items for checkmark updates
    device_name_idx: HashMap<String, usize>,
    // IDs for action items
    next_preset_id: MenuId,
    prev_preset_id: MenuId,
    random_preset_id: MenuId,
    next_device_id: MenuId,
}

impl MenuBar {
    pub fn new(
        frame_rate: u32,
        devices: &[(AudioDeviceID, String)],
        current_device_name: Option<&str>,
    ) -> Self {
        // --- App menu (first submenu = application menu on macOS) ---
        let app_menu = Submenu::new("App", true);
        app_menu
            .append_items(&[
                &PredefinedMenuItem::about(None, None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::quit(None),
            ])
            .expect("app menu construction failed");

        // --- File menu ---
        let file_menu = Submenu::new("File", true);
        file_menu
            .append_items(&[&PredefinedMenuItem::close_window(None)])
            .expect("file menu construction failed");

        // --- Presets menu ---
        let next_item = MenuItem::new(
            "Next Preset",
            true,
            Some(Accelerator::new(Some(Modifiers::META), Code::ArrowRight)),
        );
        let prev_item = MenuItem::new(
            "Previous Preset",
            true,
            Some(Accelerator::new(Some(Modifiers::META), Code::ArrowLeft)),
        );
        let random_item = MenuItem::new(
            "Random Preset",
            true,
            Some(Accelerator::new(Some(Modifiers::META), Code::KeyR)),
        );
        let next_preset_id = next_item.id().clone();
        let prev_preset_id = prev_item.id().clone();
        let random_preset_id = random_item.id().clone();

        let presets_menu = Submenu::new("Presets", true);
        presets_menu
            .append_items(&[
                &next_item,
                &prev_item,
                &PredefinedMenuItem::separator(),
                &random_item,
            ])
            .expect("presets menu construction failed");

        // --- Audio menu ---
        let next_device_item = MenuItem::new(
            "Next Input Device",
            true,
            Some(Accelerator::new(Some(Modifiers::META), Code::KeyI)),
        );
        let next_device_id = next_device_item.id().clone();

        let audio_menu = Submenu::new("Audio", true);
        audio_menu
            .append_items(&[&next_device_item, &PredefinedMenuItem::separator()])
            .expect("audio menu construction failed");

        let (device_items, device_id_map, device_name_idx) =
            Self::build_device_items(&audio_menu, devices, current_device_name);

        // --- View menu: read-only status ---
        let fps_item = MenuItem::new(format!("FPS: {}", frame_rate), false, None);
        let view_menu = Submenu::new("View", true);
        view_menu
            .append_items(&[&fps_item])
            .expect("view menu construction failed");

        // --- Root menu ---
        let menu = Menu::new();
        menu.append_items(&[
            &app_menu,
            &file_menu,
            &presets_menu,
            &audio_menu,
            &view_menu,
        ])
        .expect("menu construction failed");

        menu.init_for_nsapp();

        Self {
            _menu: menu,
            fps_item,
            device_items,
            device_id_map,
            device_name_idx,
            next_preset_id,
            prev_preset_id,
            random_preset_id,
            next_device_id,
        }
    }

    /// Update the FPS label (called once per second from the main thread).
    pub fn update_fps(&mut self, fps: u32) {
        self.fps_item.set_text(format!("FPS: {}", fps));
    }

    /// Update which device is checked (called when the active device changes).
    pub fn update_device(&self, name: &str) {
        for item in &self.device_items {
            item.set_checked(false);
        }
        if let Some(&idx) = self.device_name_idx.get(name) {
            self.device_items[idx].set_checked(true);
        }
    }

    /// Poll for a pending menu command. Returns `None` when the queue is empty.
    pub fn poll_command(&self) -> Option<MenuCommand> {
        let ev = MenuEvent::receiver().try_recv().ok()?;
        if ev.id == self.next_preset_id {
            Some(MenuCommand::NextPreset)
        } else if ev.id == self.prev_preset_id {
            Some(MenuCommand::PrevPreset)
        } else if ev.id == self.random_preset_id {
            Some(MenuCommand::RandomPreset)
        } else if ev.id == self.next_device_id {
            Some(MenuCommand::NextInputDevice)
        } else {
            self.device_id_map
                .get(&ev.id)
                .copied()
                .map(MenuCommand::SwitchAudioDevice)
        }
    }

    fn build_device_items(
        menu: &Submenu,
        devices: &[(AudioDeviceID, String)],
        current_device_name: Option<&str>,
    ) -> (
        Vec<CheckMenuItem>,
        HashMap<MenuId, AudioDeviceID>,
        HashMap<String, usize>,
    ) {
        let mut id_map = HashMap::with_capacity(devices.len());
        let mut name_idx = HashMap::with_capacity(devices.len());
        let mut items = Vec::with_capacity(devices.len());

        for (idx, (device_id, name)) in devices.iter().enumerate() {
            let checked = current_device_name == Some(name.as_str());
            let item = CheckMenuItem::new(name, true, checked, None);
            id_map.insert(item.id().clone(), *device_id);
            name_idx.insert(name.clone(), idx);
            menu.append(&item).ok();
            items.push(item);
        }

        (items, id_map, name_idx)
    }
}
