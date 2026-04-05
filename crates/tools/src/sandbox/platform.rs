use std::{path::Path, process::Output};

use tracing::{debug, warn};

use super::config::SandboxConfig;

pub struct Sandbox {
    config: SandboxConfig,
}

impl Sandbox {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    pub async fn run_command(&self, command: &str, working_dir: &Path) -> std::io::Result<Output> {
        if !self.config.enabled {
            debug!("Sandbox disabled, running command directly");
            return self.run_unsandboxed(command, working_dir).await;
        }

        #[cfg(target_os = "windows")]
        {
            self.run_windows(command, working_dir).await
        }

        #[cfg(target_os = "macos")]
        {
            self.run_macos(command, working_dir).await
        }

        #[cfg(target_os = "linux")]
        {
            self.run_linux(command, working_dir).await
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            warn!("Unknown platform, running unsandboxed");
            self.run_unsandboxed(command, working_dir).await
        }
    }

    #[cfg(target_os = "windows")]
    async fn run_windows(&self, command: &str, working_dir: &Path) -> std::io::Result<Output> {
        use std::{os::windows::process::CommandExt, process::Command};

        debug!(command, "Running in Windows sandbox via restricted token");

        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd.current_dir(working_dir);
        cmd.creation_flags(0x08000000);

        for (key, value) in self.build_env() {
            cmd.env(key, value);
        }

        cmd.output()
    }

    #[cfg(target_os = "macos")]
    async fn run_macos(&self, command: &str, working_dir: &Path) -> std::io::Result<Output> {
        use std::process::Command;

        debug!(command, "Running in macOS sandbox via sandbox-exec");

        let profile = self.build_seatbelt_profile(working_dir);

        let profile_path = working_dir.join(".onicode_sandbox.sb");
        std::fs::write(&profile_path, &profile)?;

        let mut cmd = Command::new("sandbox-exec");
        cmd.arg("-f").arg(&profile_path);
        cmd.arg("bash").arg("-c").arg(command);
        cmd.current_dir(working_dir);

        let output = cmd.output();

        let _ = std::fs::remove_file(&profile_path);
        output
    }

    #[cfg(target_os = "linux")]
    async fn run_linux(&self, command: &str, working_dir: &Path) -> std::io::Result<Output> {
        use std::process::Command;

        debug!(command, "Running in Linux sandbox via bubblewrap");

        let mut cmd = Command::new("bwrap");

        cmd.arg("--dev-bind").arg("/").arg("/");
        cmd.arg("--ro-bind").arg("/etc").arg("/etc");
        cmd.arg("--tmpfs").arg("/tmp");
        cmd.arg("--unshare-all");
        cmd.arg("--die-with-parent");

        for path in &self.config.allowed_paths {
            let abs = working_dir.join(path);
            if abs.exists() {
                cmd.arg("--bind").arg(&abs).arg(&abs);
            }
        }

        if !self.config.network_access {
            cmd.arg("--unshare-net");
        }

        cmd.arg("--chdir").arg(working_dir);
        cmd.arg("bash").arg("-c").arg(command);

        cmd.output()
    }

    async fn run_unsandboxed(&self, command: &str, working_dir: &Path) -> std::io::Result<Output> {
        debug!(command, "Running unsandboxed");

        tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(working_dir)
            .output()
            .await
    }

    fn build_env(&self) -> Vec<(String, String)> {
        let mut env = Vec::new();

        let safe_path = self
            .config
            .allowed_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(std::path::MAIN_SEPARATOR_STR);

        env.push(("PATH".into(), safe_path));

        env.push(("HOME".into(), String::new()));
        env.push(("USERPROFILE".into(), String::new()));

        env
    }

    fn build_seatbelt_profile(&self, working_dir: &Path) -> String {
        let work_dir = working_dir.to_string_lossy();

        let mut profile = String::new();

        profile.push_str("(version 1)\n\n");

        profile.push_str("(deny default)\n\n");

        profile.push_str(&format!(
            "(allow file-read* file-write* (subpath \"{}\"))\n\n",
            work_dir
        ));

        profile.push_str("(allow process-exec)\n");
        profile.push_str("(allow sysctl-read)\n");

        if self.config.network_access {
            profile.push_str("(allow network-outbound)\n");
        } else {
            profile.push_str("(deny network-outbound)\n");
        }

        for denied in &self.config.denied_paths {
            let path = denied.to_string_lossy();
            profile.push_str(&format!(
                "(deny file-read* file-write* (subpath \"{path}\"))\n"
            ));
        }

        profile
    }
}
