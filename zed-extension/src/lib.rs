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
    fn lsp_path(&self, worktree: &zed::Worktree) -> String {
        // Hardcode to the local cargo build. worktree.which() is
        // unreliable from WASM on Windows, and PATH copy/which
        // introduced more problems than it solved during dev.
        let root = worktree.root_path();
        format!("{root}/target/debug/zenlang.exe")
    }
}

zed::register_extension!(ZenlangExtension);
