// Code modified from Livesplit One Desktop. Published under no license (other related repositories by the author are under MIT), code available
// Original code by: CryZe
// Original repository: github.com/CryZe/livesplit-one-desktop
// Commit: c636ba8
use crate::formatters::{TimeFormat, TimeFormatPreset};

use livesplit_core::{
    HotkeyConfig, HotkeySystem, Run, Segment, SharedTimer, Timer, TimingMethod, auto_splitting,
    run::{parser::composite, saver::livesplit::save_timer},
};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::error;

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    window: Window,
    #[serde(default)]
    pub style: Style,
    #[serde(default)]
    pub hotkeys: HotkeyConfig,
    #[serde(default)]
    pub format: Format,
    #[serde(default)]
    connections: Connections,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct General {
    pub splits: Option<PathBuf>,
    pub timing_method: Option<TimingMethod>,
    pub comparison: Option<String>,
    pub auto_splitter: Option<PathBuf>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Style {
    pub max_segments_displayed: Option<usize>,
    pub segments_scroll_follow_from: Option<usize>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            max_segments_displayed: Some(10),
            segments_scroll_follow_from: Some(8),
        }
    }
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
#[serde(default)]
struct Window {
    always_on_top: bool,
}

#[derive(Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
#[serde(default)]
struct Connections {
    twitch: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
#[serde(default)]
pub struct Format {
    pub split: TimeFormat,
    pub timer: TimeFormat,
    pub segment: TimeFormat,
}

impl Default for Format {
    fn default() -> Self {
        Self {
            split: TimeFormat::from_preset(TimeFormatPreset::SmartDecimals),
            timer: TimeFormat::from_preset(TimeFormatPreset::ShowDecimals),
            segment: TimeFormat::from_preset(TimeFormatPreset::ShowDecimals),
        }
    }
}

impl Config {
    pub fn parse(path: impl AsRef<Path>) -> Option<Self> {
        let buf = fs::read(path).ok()?;
        serde_yaml::from_slice(&buf).ok()
    }

    pub fn parse_run(&self) -> Option<Run> {
        let path = self.general.splits.clone()?;
        let file = fs::read(&path).ok()?;
        let mut run = composite::parse(&file, Some(&path)).ok()?.run;
        run.fix_splits();
        Some(run)
    }

    pub fn parse_run_or_default(&self) -> Run {
        self.parse_run().unwrap_or_else(|| {
            let mut run = Run::new();
            run.set_game_name("Game");
            run.set_category_name("Category");
            run.push_segment(Segment::new("Time"));
            run
        })
    }

    pub fn is_game_time(&self) -> bool {
        self.general.timing_method == Some(TimingMethod::GameTime)
    }

    pub fn set_splits_path(&mut self, path: PathBuf) {
        self.general.splits = Some(path);
    }

    pub fn create_hotkey_system(&self, timer: SharedTimer) -> Option<HotkeySystem> {
        HotkeySystem::with_config(timer, self.hotkeys).ok()
    }

    pub fn configure_timer(&self, timer: &mut Timer) {
        if self.is_game_time() {
            timer.set_current_timing_method(TimingMethod::GameTime);
        }
        if let Some(comparison) = &self.general.comparison {
            timer.set_current_comparison(&**comparison).ok();
        }
    }

    pub fn save_splits(&self, timer: &Timer) {
        if let Some(path) = &self.general.splits {
            let mut buf = String::new();
            let _ = save_timer(timer, &mut buf);
            // FIXME: Don't ignore not being able to save.
            let _ = fs::write(path, &buf);
        }
    }

    pub const fn setup_logging(&self) {
        // TODO: Setup logging
        // if let Some(log) = &self.log {
        //     if let Ok(log_file) = fs::OpenOptions::new()
        //         .create(true)
        //         .write(true)
        //         .append(!log.clear)
        //         .truncate(log.clear)
        //         .open(&log.path)
        //     {
        //         fern::Dispatch::new()
        //             .format(|out, message, record| {
        //                 out.finish(format_args!(
        //                     "[{}][{}][{}] {}",
        //                     humantime::format_rfc3339_seconds(SystemTime::now()),
        //                     record.target(),
        //                     record.level(),
        //                     message
        //                 ))
        //             })
        //             .level(log.level.unwrap_or(log::LevelFilter::Warn))
        //             .chain(log_file)
        //             .apply()
        //             .ok();
        //
        //         #[cfg(not(debug_assertions))]
        //         {
        //             std::panic::set_hook(Box::new(|panic_info| {
        //                 log::error!(target: "PANIC", "{}\n{:?}", panic_info, backtrace::Backtrace::new());
        //             }));
        //         }
        //     }
        // }
    }

    pub fn maybe_load_auto_splitter(&self, runtime: &auto_splitting::Runtime) {
        if let Some(auto_splitter) = &self.general.auto_splitter
            && let Err(e) = runtime.load_script_blocking(auto_splitter.clone())
        {
            error!("Auto Splitter failed to load: {}", &e); // TODO: Create a custom error that
            // pops up in the UI
        }
    }
}
