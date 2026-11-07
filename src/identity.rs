use std::path::PathBuf;

use etcetera::base_strategy::{BaseStrategy, choose_base_strategy};
use n0_error::{Result, StdResultExt as _};

pub const ORG_NAME: &str = env!("SAILJAIL_ORGANIZATION");
pub const APP_NAME: &str = env!("SAILJAIL_APPLICATION");

pub const DBUS_NAME: &str = concat!(
    env!("SAILJAIL_ORGANIZATION"),
    ".",
    env!("SAILJAIL_APPLICATION")
);

pub fn dbus_interface() -> String {
    DBUS_NAME.replace('-', "_")
}

pub fn dbus_path() -> String {
    format!("/{}", dbus_interface().replace('.', "/"))
}

fn app_dir(base: PathBuf) -> Result<PathBuf> {
    let path = base.join(ORG_NAME).join(APP_NAME);
    std::fs::create_dir_all(&path).std_context(format!("Could not create {}", path.display()))?;
    Ok(path)
}

fn strategy() -> Result<impl BaseStrategy> {
    choose_base_strategy().std_context("No home directory for shopping list data")
}

pub fn data_dir() -> Result<PathBuf> {
    app_dir(strategy()?.data_dir())
}

pub fn config_dir() -> Result<PathBuf> {
    app_dir(strategy()?.config_dir())
}
