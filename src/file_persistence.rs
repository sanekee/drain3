use crate::persistence::PersistenceHandler;
use std::fs;
use std::path::Path;
use anyhow::Result;

pub struct FilePersistence {
    pub file_path: String,
}

impl FilePersistence {
    pub fn new(file_path: String) -> Self {
        Self { file_path }
    }
}

impl PersistenceHandler for FilePersistence {
    fn save_state(&mut self, state: &[u8]) -> Result<()> {
        fs::write(&self.file_path, state)?;
        Ok(())
    }

    fn load_state(&mut self) -> Result<Option<Vec<u8>>> {
        if Path::new(&self.file_path).exists() {
            let data = fs::read(&self.file_path)?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }
}