use std::collections::HashMap;
use std::time::SystemTime;

use crate::compiler;
use crate::error::{Error, Result};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::resolver;
use crate::stdlib;
use crate::typeck;
use crate::vm::VM;

/// Watches script source files and hot-reloads the VM when changes are detected.
///
/// On each `tick()` or `force_reload()`, checks if any watched file has been
/// modified. If so, re-lexes, re-parses, re-resolves, re-typechecks, and
/// re-compiles the script, then migrates surviving global state into the
/// new bytecode.
pub struct HotReloader {
    script_paths: Vec<std::path::PathBuf>,
    mtimes: HashMap<std::path::PathBuf, SystemTime>,
    vm: VM,
    last_source: Option<String>,
}

impl HotReloader {
    /// Create a new `HotReloader` that watches the given script files.
    pub fn new(script_paths: impl IntoIterator<Item = impl Into<std::path::PathBuf>>, vm: VM) -> Self {
        let paths: Vec<_> = script_paths.into_iter().map(Into::into).collect();
        let mtimes = paths.iter().filter_map(|p| {
            std::fs::metadata(p).ok().and_then(|m| m.modified().ok()).map(|t| (p.clone(), t))
        }).collect();
        Self { script_paths: paths, mtimes, vm, last_source: None }
    }

    /// Check for source file changes and reload if any occurred.
    ///
    /// Returns `true` if a reload happened, `false` otherwise.
    pub fn tick(&mut self) -> Result<bool> {
        if self.any_file_changed() {
            self.do_reload()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Force a full reload regardless of file modification times.
    pub fn force_reload(&mut self) -> Result<()> {
        self.do_reload()
    }

    /// Get a reference to the VM.
    pub fn vm(&self) -> &VM {
        &self.vm
    }

    /// Get a mutable reference to the VM.
    pub fn vm_mut(&mut self) -> &mut VM {
        &mut self.vm
    }

    /// Read the current source from the first script path.
    fn read_source(&self) -> Result<String> {
        let path = self.script_paths.first()
            .ok_or_else(|| Error::Runtime {
                msg: "no script path configured for hot reload".into(),
                stack_trace: Vec::new(),
            })?;
        std::fs::read_to_string(path).map_err(|e| Error::Io { source: e })
    }

    fn any_file_changed(&mut self) -> bool {
        let mut changed = false;
        for path in &self.script_paths {
            let mtime = std::fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok());
            let previous = self.mtimes.get(path);
            match (mtime, previous) {
                (Some(t), Some(prev)) if t > *prev => {
                    self.mtimes.insert(path.clone(), t);
                    changed = true;
                }
                (Some(t), None) => {
                    self.mtimes.insert(path.clone(), t);
                    changed = true;
                }
                _ => {}
            }
        }
        changed
    }

    fn do_reload(&mut self) -> Result<()> {
        let source = self.read_source()?;

        // Skip if source hasn't changed since last compile
        if self.last_source.as_deref() == Some(&source) {
            return Ok(());
        }

        tracing::info!("hot reload: recompiling script");

        // Run the full compilation pipeline
        let tokens = Lexer::new(&source).tokenize()?;
        let parser = Parser::new(&tokens);
        let mut program = parser.parse()?;

        let native_names = stdlib::native_names();
        let mut symbols = resolver::resolve_with_natives(&mut program, &native_names)?;
        let types = typeck::check(&program, &mut symbols)?;
        let (fns, global_names) = compiler::compile(&program, &types, &symbols, &native_names, &source)?;

        // Swap bytecode while migrating global state
        self.vm.reload_functions(fns, global_names)?;

        self.last_source = Some(source);
        tracing::info!("hot reload: success");
        Ok(())
    }
}
