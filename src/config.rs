use std::{
    collections::HashMap,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use tokio::fs;

#[derive(Debug)]
pub struct Service {
    pub token: String,
    pub path: PathBuf,
}

#[derive(Debug, Default)]
pub struct Config {
    pub services: HashMap<String, Service>,
}

impl Config {
    pub async fn load(&mut self) -> anyhow::Result<()> {
        let path =
            std::env::var("CONF_PATH").map_err(|_| anyhow::anyhow!("no CONF_PATH defined"))?;
        tracing::info!("loading config");
        let content = match fs::read_to_string(std::path::Path::new(&path)).await {
            Ok(content) => content,
            Err(e) => {
                tracing::error!("could not open file '{}': {}", &path, e);
                return Err(anyhow::anyhow!("could not open file '{}': {}", &path, e));
            }
        };
        let mut services: HashMap<String, Service> = HashMap::new();
        let mut error_count = 0;
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split(";").map(str::trim).collect();
            if parts.len() != 3 {
                tracing::error!(
                    "invalid config line, expected 'SERVICE;TOKEN;PATH', received '{line}'"
                );
                error_count += 1;
                continue;
            }
            if services.contains_key(parts[0]) {
                tracing::warn!(
                    "service '{}' already exists, line skipped: '{}'",
                    parts[0],
                    line,
                );
            } else {
                let path = PathBuf::from(parts[1]);
                let _ = check_path(&path);
                services.insert(
                    parts[0].to_string(),
                    Service {
                        path,
                        token: parts[2].to_string(),
                    },
                );
            }
        }

        if error_count > 0 {
            tracing::info!(
                "{} error{} occuried, config not loaded",
                error_count,
                if error_count > 1 { "s" } else { "" }
            );
        } else {
            tracing::info!("config loaded");
            self.services = services;
        }

        let keys = self.services.keys();
        tracing::info!(
            "current config has {} service{}{} {}",
            self.services.len(),
            if self.services.len() > 1 { "s" } else { "" },
            if !self.services.is_empty() { ":" } else { "" },
            keys.map(String::as_str).collect::<Vec<_>>().join(", ")
        );
        Ok(())
    }
}

fn check_path(path: &Path) -> Result<(), std::io::Error> {
    let content = match std::fs::read(path) {
        Ok(content) => content,
        Err(e) => {
            tracing::warn!("io error for {:?}: {}", path, e);
            return Err(e);
        }
    };
    if !content.starts_with(b"#!") {
        tracing::warn!("{:?} is missing shelang", path);
    };
    if std::fs::metadata(path)?.permissions().mode() & 0o111 == 0 {
        tracing::warn!("{:?} isn't executable, check permission", path);
    }
    Ok(())
}
