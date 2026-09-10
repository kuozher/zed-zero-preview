use std::fs;
use zed::LanguageServerId;
use zed_extension_api::{self as zed, Result, settings::LspSettings};

struct ZeroPreviewExtension {
    cached_binary_path: Option<String>,
}

impl ZeroPreviewExtension {
    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        if let Ok(settings) = LspSettings::for_worktree("zero-preview", worktree) {
            if let Some(binary) = settings.binary {
                if let Some(path) = binary.path {
                    if fs::metadata(&path).map_or(false, |stat| stat.is_file()) {
                        return Ok(path);
                    }
                }
            }
        }

        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).map_or(false, |stat| stat.is_file()) {
                return Ok(path.clone());
            }
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = zed::latest_github_release(
            "kuozher/zed-zero-preview",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let (platform, arch) = zed::current_platform();

        let asset_name = format!(
            "zero-preview-server-{arch}-{os}",
            arch = match arch {
                zed::Architecture::Aarch64 => "aarch64",
                zed::Architecture::X86 => "x86",
                zed::Architecture::X8664 => "x86_64",
            },
            os = match platform {
                zed::Os::Mac => "apple-darwin",
                zed::Os::Linux => "unknown-linux-musl",
                zed::Os::Windows => "pc-windows-msvc.exe",
            },
        );

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("no asset found matching {:?}", asset_name))?;

        let version_dir = format!("zero-preview-{}", release.version);
        fs::create_dir_all(&version_dir)
            .map_err(|err| format!("failed to create directory '{version_dir}': {err}"))?;

        let binary_path = format!("{version_dir}/{asset_name}");

        if !fs::metadata(&binary_path).map_or(false, |stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(
                &asset.download_url,
                &binary_path,
                zed::DownloadedFileType::Uncompressed,
            )
            .map_err(|err| format!("failed to download file: {err}"))?;

            zed::make_file_executable(&binary_path)?;
        }

        let entries = fs::read_dir(".")
            .map_err(|err| format!("failed to list working directory {err}"))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("failed to load directory entry {err}"))?;
            if entry.file_name().to_str() != Some(&version_dir) {
                fs::remove_dir_all(&entry.path()).ok();
            }
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }
}

impl zed::Extension for ZeroPreviewExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        Ok(zed::Command {
            command: self.language_server_binary_path(language_server_id, worktree)?,
            args: vec![],
            env: Default::default(),
        })
    }
}

zed::register_extension!(ZeroPreviewExtension);
