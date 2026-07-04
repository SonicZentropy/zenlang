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
    fn exe_name(&self) -> &'static str {
        if zed::current_platform().0 == Os::Windows {
            "zenc.exe"
        } else {
            "zenc"
        }
    }

    fn lsp_path(&self, worktree: &zed::Worktree) -> String {
        if let Some(path) = worktree.which(self.exe_name()) {
            return path;
        }
        format!("{}/target/debug/{}", worktree.root_path(), self.exe_name())
    }
}

zed::register_extension!(ZenlangExtension);
