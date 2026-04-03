mod app;
mod dummy_audio;
use std::path::PathBuf;

use crate::app::config::Config;
use clap::Parser;
use confique::Config as ConfiqueConfig;

// User specified configuration options.
//
// Defines CLI, env, and config file parameters.
#[derive(Parser, ConfiqueConfig, Clone, Debug)]
#[command(version)]
/// ProjectM: the milkdrop-compatible music visualizer.
///
/// Need help? Join discord: https://discord.gg/uSSggaMBrv
struct Settings {
    #[arg(short, long = "config")]
    #[arg(default_value = "Contents/Resources/config.toml")]
    /// Path to a config file
    config_path: Option<PathBuf>,

    #[arg(short, long)]
    #[arg(env = "PM_FRAME_RATE")]
    /// Frame rate to render at.
    frame_rate: Option<u32>,

    #[arg(short, long)]
    #[arg(env = "PM_PRESET_PATH")]
    /// Path to preset directory
    preset_path: Option<PathBuf>,

    #[arg(short, long)]
    #[arg(env = "PM_TEXTURE_PATH")]
    /// Path to texture directory
    texture_path: Option<PathBuf>,

    #[arg(short, long)]
    #[arg(env = "PM_BEAT_SENSITIVITY")]
    /// Sensitivity of the beat detection
    beat_sensitivity: Option<f32>,

    #[arg(short = 'd', long)]
    #[arg(env = "PM_PRESET_DURATION")]
    /// Duration (seconds) each preset will play
    preset_duration: Option<f64>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            config_path: None,
            frame_rate: Some(60),
            preset_path: None,
            texture_path: None,
            beat_sensitivity: Some(1.0),
            preset_duration: Some(10.0),
        }
    }
}

impl Settings {
    // Overrides `self` with values of `other`, if they exist
    fn apply(&mut self, other: &Settings) {
        if let Some(config_path) = &other.config_path {
            self.config_path = Some(config_path.clone());
        }
        if let Some(frame_rate) = other.frame_rate {
            self.frame_rate = Some(frame_rate);
        }
        if let Some(preset_path) = &other.preset_path {
            self.preset_path = Some(preset_path.clone());
        }
        if let Some(texture_path) = &other.texture_path {
            self.texture_path = Some(texture_path.clone());
        }
        if let Some(beat_sensitivity) = other.beat_sensitivity {
            self.beat_sensitivity = Some(beat_sensitivity);
        }
        if let Some(preset_duration) = other.preset_duration {
            self.preset_duration = Some(preset_duration);
        }
    }
}

fn load_settings_file(path: Option<PathBuf>) -> Result<Settings, String> {
    // Load file config if a path is specified
    if let Some(path) = path {
        // ensure the path exists
        if !path.exists() {
            // If it's the default path and doesn't exist, just return empty settings
            // This allows running without a config file if the default is missing
            return Ok(Settings::default());
        }
        // ensure extention is valid
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("toml") | Some("json") | Some("yaml)") => {}
            _ => {
                return Err(format!(
                    "invalid config file extension: {:?}",
                    path.extension()
                ));
            }
        }

        println!("Loading config from: {}", path.display());

        // Load setting from file
        let settings = Settings::builder()
            .file(path)
            .load()
            .map_err(|e| e.to_string())?;

        return Ok(settings);
    }

    // No path, return empty settings
    return Ok(Settings::default());
}

fn load_settings() -> Result<Settings, String> {
    // Load CLI flags and env vars
    let cli = Settings::parse();

    // Load file
    let mut settings = load_settings_file(cli.config_path.clone())?;

    // Override file with CLI/env vars
    settings.apply(&cli);

    // Apply global defaults if still None
    if settings.frame_rate.is_none() {
        settings.frame_rate = Some(60);
    }
    if settings.beat_sensitivity.is_none() {
        settings.beat_sensitivity = Some(1.0);
    }
    if settings.preset_duration.is_none() {
        settings.preset_duration = Some(10.0);
    }

    return Ok(settings);
}

fn main() -> Result<(), String> {
    // Set the process name before anything else so the macOS menu bar shows
    // "ProjectM" instead of the binary name "projectm_sdl".
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        unsafe extern "C" {
            fn setprogname(name: *const std::ffi::c_char);
        }
        let name = CString::new("ProjectM").unwrap();
        unsafe { setprogname(name.as_ptr()) };
    }

    let settings = load_settings()?;

    let mut app_config = Config::default();
    if let Some(frame_rate) = settings.frame_rate {
        app_config.frame_rate = Some(frame_rate);
    }
    if let Some(preset_path) = settings.preset_path {
        app_config.preset_path = Some(preset_path);
    }
    if let Some(texture_path) = settings.texture_path {
        app_config.texture_path = Some(texture_path);
    }
    if let Some(beat_sensitivity) = settings.beat_sensitivity {
        app_config.beat_sensitivity = Some(beat_sensitivity);
    }
    if let Some(preset_duration) = settings.preset_duration {
        app_config.preset_duration = Some(preset_duration);
    }

    // Initialize the application
    let mut app = app::App::new(app_config);
    app.init();
    app.main_loop();

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::Settings;
    use clap::Parser;
    use confique::Config;

    fn assert_settings(s: Settings) {
        assert_eq!(s.frame_rate, Some(60));
        assert_eq!(
            s.preset_path.as_ref().map(|p| p.to_str().unwrap()),
            Some("/home/user/.local/share/projectm/presets")
        );
        assert_eq!(
            s.texture_path.as_ref().map(|p| p.to_str().unwrap()),
            Some("/home/user/.local/share/projectm/textures")
        );
        assert_eq!(s.beat_sensitivity, Some(1.0));
        assert_eq!(s.preset_duration, Some(10.0));
    }

    #[test]
    fn test_load_toml() {
        let res = Settings::builder()
            .file("test-data/config.toml")
            .load()
            .expect("TOML settings should load");

        assert_settings(res);
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_load_env_vars() {
        unsafe {
            std::env::set_var("PM_FRAME_RATE", "60");
            std::env::set_var("PM_PRESET_PATH", "/home/user/.local/share/projectm/presets");
            std::env::set_var(
                "PM_TEXTURE_PATH",
                "/home/user/.local/share/projectm/textures",
            );
            std::env::set_var("PM_BEAT_SENSITIVITY", "1.0");
            std::env::set_var("PM_PRESET_DURATION", "10.0");
            std::env::set_var("PM_AUDIO_INPUT", "default");
        };

        // Environment variables are loaded through CLI parsing in clap
        // Create a fake CLI args with no arguments to trigger env var loading
        let args = vec!["test_program"];
        let res = Settings::try_parse_from(args)
            .expect("Environment variable settings should load through CLI parsing");

        assert_settings(res);

        // Clean up environment variables
        unsafe {
            std::env::remove_var("PM_FRAME_RATE");
            std::env::remove_var("PM_PRESET_PATH");
            std::env::remove_var("PM_TEXTURE_PATH");
            std::env::remove_var("PM_BEAT_SENSITIVITY");
            std::env::remove_var("PM_PRESET_DURATION");
            std::env::remove_var("PM_AUDIO_INPUT");
        }
    }
}
