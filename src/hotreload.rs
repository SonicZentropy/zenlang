// Phase 8: Hot reloading
// TODO: File watcher, state migration, VM swap

use crate::error::Result;
use crate::vm::VM;

pub struct HotReloader;

impl HotReloader {
    pub fn new(_script_paths: &[camino::Utf8PathBuf]) -> Self {
        Self
    }

    pub fn tick(&mut self) -> Result<&mut VM> {
        // Placeholder
        unimplemented!()
    }

    pub fn force_reload(&mut self) -> Result<&mut VM> {
        // Placeholder
        unimplemented!()
    }
}
