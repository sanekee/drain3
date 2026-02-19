use anyhow::Result;

pub trait PersistenceHandler {
    fn save_state(&mut self, state: &[u8]) -> Result<()>;
    fn load_state(&mut self) -> Result<Option<Vec<u8>>>;
}

