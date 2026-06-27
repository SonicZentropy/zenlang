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
        let path = worktree
            .which("zenlang")
            .ok_or_else(|| "zenlang binary not found on PATH. Install it with `cargo install --path .` in the zenlang project directory.".to_string())?;

        Ok(zed::Command {
            command: path,
            args: vec!["lsp".to_string()],
            env: Default::default(),
        })
    }
}

zed::register_extension!(ZenlangExtension);
