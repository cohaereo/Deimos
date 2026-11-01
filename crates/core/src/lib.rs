use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use tiger_pkg::PackageManager;
use tracing::info;
pub mod config;

pub const MARATHON_APP_ID: u64 = 3065800;

pub fn initialize_package_manager<'a>(
    suggested_path: impl Into<Option<&'a str>>,
) -> anyhow::Result<()> {
    let game_path = if let Some(path) = &suggested_path.into() {
        (*path).to_string()
    } else {
        let Some(steamapp) = game_detector::steam::get_all_apps()
            .context("Failed to enumerate Steam apps")?
            .into_iter()
            .find(|a| a.appid == MARATHON_APP_ID)
        else {
            panic!("Failed to find Marathon app in Steam library. If you don't have Marathon installed through Steam, then you can specify the path to the game directory using the --gamedir/-g argument.");
        };

        info!("Found Marathon installation at '{}'", steamapp.game_path);

        steamapp.game_path
    };

    let pm = Arc::new(
        PackageManager::new(
            PathBuf::from(&game_path).join("packages"),
            tiger_pkg::GameVersion::Marathon(tiger_pkg::MarathonVersion::Marathon),
            None,
        )
        .expect("Failed to initialize package manager"),
    );
    tiger_pkg::initialize_package_manager(&pm);
    Ok(())
}
