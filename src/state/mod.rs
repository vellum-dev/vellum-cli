use std::fs;
use std::path::PathBuf;

use anyhow::Result;

pub struct State {
    root: PathBuf,
}

impl State {
    pub fn new(vellum_root: &str) -> Self {
        Self {
            root: PathBuf::from(vellum_root),
        }
    }

    fn dir(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn get_os_version(&self) -> Result<String> {
        let data = fs::read_to_string(self.dir().join("osver"))?;
        Ok(data.trim().to_string())
    }

    pub fn set_os_version(&self, version: &str) -> Result<()> {
        fs::create_dir_all(self.dir())?;
        fs::write(self.dir().join("osver"), version)?;
        Ok(())
    }

    pub fn get_device(&self) -> Result<String> {
        let data = fs::read_to_string(self.dir().join("device"))?;
        Ok(data.trim().to_string())
    }

    pub fn set_device(&self, device: &str) -> Result<()> {
        fs::create_dir_all(self.dir())?;
        fs::write(self.dir().join("device"), device)?;
        Ok(())
    }
}
