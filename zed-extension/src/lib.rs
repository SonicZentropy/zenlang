use zed_extension_api as zed;

struct ZenlangExtension;

impl zed::Extension for ZenlangExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let path = self.lsp_path(worktree);
        Ok(zed::Command {
            command: path,
            args: vec!["lsp".to_string()],
            env: Default::default(),
        })
    }
}

impl ZenlangExtension {
    /// Locate the `zenlang` binary.
    ///
    /// Inside WASM we cannot use `std::path::Path::exists()` (WASI lacks
    /// filesystem access), so we construct the path without checking.
    ///
    /// 1. `zenlang` on `$PATH`
    /// 2. `{worktree_root}/target/debug/zenlang` (or `.exe` on Windows)
    /// 3. `{worktree_root}/target/release/zenlang`
    fn lsp_path(&self, worktree: &zed::Worktree) -> String {
        // Priority 1 — on PATH
        if let Some(p) = worktree.which("zenlang") {
            return p;
        }

        // Priority 2 — local cargo build
        let root = worktree.root_path();
        let exe = if cfg!(windows) { "zenlang.exe" } else { "zenlang" };
        format!("{root}/target/debug/{exe}")
    }
}

zed::register_extension!(ZenlangExtension);
