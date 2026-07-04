use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::compiler;
use crate::error::{Error, Result};
use crate::lexer::Lexer;
use crate::mod_resolver;
use crate::parser::Parser;
use crate::resolver;
use crate::stdlib;
use crate::typeck;
use crate::vm::VM;

/// Watches script source files and hot-reloads the VM when changes are detected.
///
/// On each `tick()` or `force_reload()`, checks if any watched file has been
/// modified. If so, re-lexes, re-parses, re-resolves, re-typechecks, and
/// re-compiles the *entire project* (the root script plus every file-backed
/// `mod` it transitively pulls in), then migrates surviving global state into
/// the new bytecode.
///
/// The watch set automatically grows and shrinks as `mod name;` declarations
/// are added or removed from the project, so editing any file that's part of
/// the module graph — not just the entry script — triggers a reload.
pub struct HotReloader {
    /// The entry script. Always recompiled from here on every reload.
    root_path: PathBuf,
    /// Extra files to watch beyond the module graph rooted at `root_path`
    /// (as originally passed to `new`, for backwards compatibility with
    /// callers that watch files not reachable via `mod`).
    extra_paths: Vec<PathBuf>,
    /// Every currently-watched file (root + extras + discovered `mod` files)
    /// mapped to the mtime it had as of the last successful reload check.
    mtimes: HashMap<PathBuf, SystemTime>,
    vm: VM,
}

impl HotReloader {
    /// Create a new `HotReloader` that watches the given script files.
    ///
    /// The first path is treated as the project's entry script (the one
    /// compiled and run); any additional paths are watched alongside it.
    /// Files pulled in via file-backed `mod name;` declarations from the
    /// entry script are discovered and watched automatically — they don't
    /// need to be listed here.
    pub fn new(script_paths: impl IntoIterator<Item = impl Into<PathBuf>>, vm: VM) -> Self {
        let mut paths = script_paths.into_iter().map(Into::into);
        let root_path = paths.next().unwrap_or_default();
        let extra_paths: Vec<PathBuf> = paths.collect();
        let mut reloader = Self {
            root_path,
            extra_paths,
            mtimes: HashMap::new(),
            vm,
        };
        // Best-effort initial discovery so the first `tick()` doesn't
        // spuriously report every file as "changed".
        let _ = reloader.refresh_watch_set();
        reloader
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

    /// The set of files currently being watched (entry script, any extra
    /// paths passed to `new`, and every file-backed `mod` discovered on the
    /// last successful reload).
    pub fn watched_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.mtimes.keys()
    }

    fn any_file_changed(&mut self) -> bool {
        let mut changed = false;
        // Snapshot the current key set so we can also notice brand-new
        // files (e.g. a `mod foo;` line was just added but `foo.zen` didn't
        // exist yet at last reload, or an mtime disappeared because the
        // file was deleted).
        let paths: Vec<PathBuf> = self.mtimes.keys().cloned().collect();
        for path in &paths {
            let mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
            let previous = self.mtimes.get(path).copied();
            match (mtime, previous) {
                (Some(t), Some(prev)) if t > prev => changed = true,
                (None, Some(_)) => changed = true, // file removed/unreadable
                _ => {}
            }
        }
        changed
    }

    /// Re-scan the module graph rooted at `root_path` and rebuild the
    /// watched-file mtime map to match exactly (root + extras + discovered
    /// `mod` files). Called after every successful reload so files added or
    /// removed from the `mod` graph are picked up automatically.
    fn refresh_watch_set(&mut self) -> Result<()> {
        let mut watch_paths = vec![self.root_path.clone()];
        watch_paths.extend(self.extra_paths.iter().cloned());

        if let Ok(source) = std::fs::read_to_string(&self.root_path)
            && let Ok(tokens) = Lexer::new(&source).tokenize()
            && let Ok(mut program) = Parser::new(&source, &tokens).parse()
            && let Ok(mod_paths) =
                mod_resolver::resolve_modules_with_paths(&mut program, &self.root_path)
        {
            watch_paths.extend(mod_paths);
        }

        let mut new_mtimes = HashMap::new();
        for path in watch_paths {
            let mtime = std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok());
            if let Some(t) = mtime {
                new_mtimes.insert(path, t);
            }
        }
        self.mtimes = new_mtimes;
        Ok(())
    }

    fn do_reload(&mut self) -> Result<()> {
        tracing::info!("hot reload: recompiling project from {:?}", self.root_path);

        let source =
            std::fs::read_to_string(&self.root_path).map_err(|e| Error::Io { source: e })?;

        // Run the full compilation pipeline, resolving the whole `mod` graph
        // (not just the entry script) so multi-file projects hot-reload
        // correctly.
        let tokens = Lexer::new(&source).tokenize()?;
        let parser = Parser::new(&source, &tokens);
        let mut program = parser.parse()?;
        mod_resolver::resolve_modules(&mut program, &self.root_path)?;
        crate::prelude::inject(&mut program)?;

        let native_names = stdlib::native_names();
        let mut symbols = resolver::resolve_with_natives(&mut program, &native_names)?;
        let types = typeck::check(&program, &mut symbols)?;
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &native_names, &source)?;

        // Swap bytecode while migrating global state
        self.vm.reload_functions(fns, global_names)?;

        // Give the script a chance to react to the reload — e.g. re-derive
        // a cached lookup map, or reset a timer — beyond what plain
        // global-value snapshotting can do on its own. Purely optional: a
        // script that doesn't define `on_reload` is unaffected.
        self.vm.call_if_exists("on_reload")?;

        // Re-discover the watch set now that we know the current module
        // graph (files may have been added/removed by this edit).
        self.refresh_watch_set()?;

        tracing::info!("hot reload: success");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::{native_names, register_builtins};

    fn compile_and_run(root: &std::path::Path) -> VM {
        let source = std::fs::read_to_string(root).unwrap();
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let mut program = Parser::new(&source, &tokens).parse().unwrap();
        mod_resolver::resolve_modules(&mut program, root).unwrap();
        let names = native_names();
        let mut symbols = resolver::resolve_with_natives(&mut program, &names).unwrap();
        let types = typeck::check(&program, &mut symbols).unwrap();
        let (fns, global_names) =
            compiler::compile(&program, &types, &symbols, &names, &source).unwrap();
        let mut vm = VM::new();
        register_builtins(&mut vm);
        vm.load_bytecode(fns, global_names);
        vm.run_main().unwrap();
        vm
    }

    /// Editing a file-backed `mod` (not just the entry script) must trigger
    /// a reload, and the reload must re-resolve the whole module graph.
    #[test]
    fn test_reload_picks_up_submodule_change() {
        let dir = std::env::temp_dir().join(format!("zen_hotreload_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let root_path = dir.join("main.zen");
        let sub_path = dir.join("combat.zen");

        std::fs::write(&sub_path, "fn damage() -> i64 { 10 }\n").unwrap();
        std::fs::write(
            &root_path,
            "mod combat;\nuse combat::damage;\nlet hits = 0;\nfn main() { hits = hits + 1; damage() }\n",
        )
        .unwrap();

        let vm = compile_and_run(&root_path);
        let mut reloader = HotReloader::new([root_path.clone()], vm);

        // combat.zen must already be part of the watch set from construction.
        assert!(reloader.watched_paths().any(|p| p == &sub_path));

        // No changes yet.
        assert!(!reloader.tick().unwrap());

        // Bump the submodule's mtime forward and change its content —
        // this is the file the bug report says gets silently ignored.
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&sub_path, "fn damage() -> i64 { 99 }\n").unwrap();

        let reloaded = reloader.tick().unwrap();
        assert!(reloaded, "editing a submodule file should trigger a reload");

        let result = reloader.vm_mut().run_main().unwrap();
        assert_eq!(result, crate::value::Value::Int(99));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// If the script defines `fn on_reload()`, it must be called exactly
    /// once after each successful reload, with the *new* bytecode already
    /// live (so it can call other new functions / see restored globals).
    #[test]
    fn test_reload_calls_on_reload_hook_if_defined() {
        let dir = std::env::temp_dir().join(format!("zen_hotreload_test3_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let root_path = dir.join("main.zen");

        std::fs::write(
            &root_path,
            "let reload_count = 0;\nfn main() { reload_count }\n",
        )
        .unwrap();

        let vm = compile_and_run(&root_path);
        let mut reloader = HotReloader::new([root_path.clone()], vm);

        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(
            &root_path,
            "let reload_count = 0;\nfn on_reload() { reload_count = reload_count + 1; }\nfn get_reload_count() -> i64 { reload_count }\nfn main() { reload_count }\n",
        )
        .unwrap();
        assert!(reloader.tick().unwrap());
        // `on_reload` ran once as part of this reload. Check via a plain
        // accessor function rather than `run_main()`, since `run_main()`
        // re-executes the whole top-level program (including `let
        // reload_count = 0;`) each time it's called, which would mask the
        // hook's effect.
        let count = reloader
            .vm_mut()
            .call_if_exists("get_reload_count")
            .unwrap();
        assert_eq!(count, Some(crate::value::Value::Int(1)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A script with no `on_reload` defined must reload without error —
    /// the hook is entirely optional.
    #[test]
    fn test_reload_without_on_reload_hook_is_fine() {
        let dir = std::env::temp_dir().join(format!("zen_hotreload_test4_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let root_path = dir.join("main.zen");

        std::fs::write(&root_path, "let x = 1;\nfn main() { x }\n").unwrap();
        let vm = compile_and_run(&root_path);
        let mut reloader = HotReloader::new([root_path.clone()], vm);

        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&root_path, "let x = 2;\nfn main() { x }\n").unwrap();
        assert!(reloader.tick().unwrap());
        assert_eq!(
            reloader.vm_mut().run_main().unwrap(),
            crate::value::Value::Int(2)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Adding a new `mod` declaration and its backing file after the
    /// reloader was created should be picked up on the next reload, and the
    /// new file should then also be watched.
    #[test]
    fn test_reload_discovers_new_module_file() {
        let dir = std::env::temp_dir().join(format!("zen_hotreload_test2_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let root_path = dir.join("main.zen");

        std::fs::write(&root_path, "let x = 1;\nfn main() { x }\n").unwrap();

        let vm = compile_and_run(&root_path);
        let mut reloader = HotReloader::new([root_path.clone()], vm);

        let extra_path = dir.join("extra.zen");
        std::fs::write(&extra_path, "fn extra_val() -> i64 { 7 }\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(
            &root_path,
            "mod extra;\nuse extra::extra_val;\nlet x = 1;\nfn main() { extra_val() }\n",
        )
        .unwrap();

        assert!(reloader.tick().unwrap());
        assert!(reloader.watched_paths().any(|p| p == &extra_path));
        let result = reloader.vm_mut().run_main().unwrap();
        assert_eq!(result, crate::value::Value::Int(7));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
