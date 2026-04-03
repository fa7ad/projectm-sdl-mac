use std::path::PathBuf;

use crate::app::App;

impl App {
    /// Add presets to the playlist recursively skipping duplicates.
    pub fn add_preset_path(&self, preset_path: &PathBuf) {
        self.playlist.add_path(preset_path.to_str().unwrap(), true);
        println!("added preset path: {}", preset_path.to_str().unwrap());
        println!("playlist size: {}", self.playlist.len());
    }
}
