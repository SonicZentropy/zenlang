use zed_extension_api::{self as zed, Os};

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
        let root = worktree.root_path();
        let exe = if zed::current_platform().0 == Os::Windows {
            "zenc.exe"
        } else {
            "zenc"
        };
        format!("{root}/target/debug/{exe}")
    }
}

zed::register_extension!(ZenlangExtension);
