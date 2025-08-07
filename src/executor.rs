// src/executor.rs
use crate::schema::*;
use crate::template::Renderer;
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::path::Path;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

pub struct Executor {
    renderer: Renderer,
    ctx: Value,
    #[allow(dead_code)]
    verbose: bool,
    dry_run: bool,
}

impl Executor {
    pub fn new(globals: Value, verbose: bool, dry_run: bool) -> Self {
        Self {
            renderer: Renderer::new(),
            ctx: globals,
            verbose,
            dry_run,
        }
    }

    pub async fn run_step(&self, step: &Step, idx: usize) -> Result<()> {
        if let Some(false) = step.when {
            return Ok(());
        }

        if let Some(shell) = &step.shell {
            self.run_shell(step, shell, idx).await
        } else if let Some(exec) = &step.exec {
            self.run_exec(step, exec, idx).await
        } else if let Some(conf) = &step.conf {
            self.run_conf(step, conf, idx).await
        } else if let Some(ssh) = &step.ssh {
            self.run_ssh(step, ssh, idx).await
        } else {
            anyhow::bail!("Step {} hat keinen ausführbaren Block", idx)
        }
    }

    async fn run_shell(&self, step: &Step, spec: &ShellSpec, idx: usize) -> Result<()> {
        let cmd_str = self.renderer.render_str(&spec.command, &self.ctx)?;
        let shell = spec.shell.clone().unwrap_or_else(|| "bash -c".into());
        let mut parts = shell
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let (prg, mut args) = (parts.remove(0), parts);
        args.push(cmd_str.clone());

        let env = self.merge_env(&step.env, &spec.env)?;
        self.print_header(idx, step.name.as_deref().unwrap_or("shell"), &cmd_str);

        if self.dry_run {
            return Ok(());
        }

        let mut child = Command::new(&prg)
            .args(&args)
            .envs(env)
            .current_dir(spec.cwd.clone().unwrap_or_else(|| ".".into()))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("shell spawn")?;

        self.stream_child(&mut child, "shell").await
    }

    async fn run_exec(&self, step: &Step, spec: &ExecSpec, idx: usize) -> Result<()> {
        let cmd = self.renderer.render_str(&spec.cmd, &self.ctx)?;
        let args = spec
            .args
            .iter()
            .map(|a| self.renderer.render_str(a, &self.ctx))
            .collect::<Result<Vec<_>>>()?;
        let env = self.merge_env(&step.env, &spec.env)?;
        let line = format!("{} {}", cmd, shell_escape::escape(args.join(" ").into()));
        self.print_header(idx, step.name.as_deref().unwrap_or("exec"), &line);

        if self.dry_run {
            return Ok(());
        }

        let mut child = Command::new(&cmd)
            .args(&args)
            .envs(env)
            .current_dir(spec.cwd.clone().unwrap_or_else(|| ".".into()))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("exec spawn")?;

        self.stream_child(&mut child, "exec").await
    }

    async fn run_conf(&self, step: &Step, spec: &ConfSpec, idx: usize) -> Result<()> {
        let dest = self.renderer.render_str(&spec.dest, &self.ctx)?;
        let content = self.renderer.render_str(&spec.template, &self.ctx)?;
        self.print_header(
            idx,
            step.name.as_deref().unwrap_or("conf"),
            &format!("write {}", dest),
        );

        if self.dry_run {
            println!("Content preview:\n{}", content);
            return Ok(());
        }

        let path = Path::new(&dest);
        if spec.backup && path.exists() {
            let bak = format!("{}.bak", dest);
            std::fs::copy(&dest, &bak).context("backup copy")?;
            println!("[conf] backup -> {}", bak);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, content)?;
        if let Some(mode) = &spec.mode {
            use std::os::unix::fs::PermissionsExt;
            let m = u32::from_str_radix(mode, 8).unwrap_or(0o644);
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(m))?;
        }
        Ok(())
    }

    async fn run_ssh(&self, step: &Step, spec: &SshSpec, idx: usize) -> Result<()> {
        // Variante A: openssh crate, nutzt lokales ssh
        let host = self.renderer.render_str(&spec.host, &self.ctx)?;
        let user = if let Some(u) = &spec.user {
            self.renderer.render_str(u, &self.ctx)?
        } else {
            "root".to_string()
        };
        let command = self.renderer.render_str(&spec.command, &self.ctx)?;
        let env = self.renderer.render_map(&spec.env, &self.ctx)?;

        let mut ssh_cmd = vec!["ssh".to_string()];
        match spec.check_host.as_deref() {
            Some("no") | None => ssh_cmd.extend(
                [
                    "-o",
                    "StrictHostKeyChecking=no",
                    "-o",
                    "UserKnownHostsFile=/dev/null",
                ]
                .map(String::from),
            ),
            Some("yes") => {}
            Some("fingerprint") => {} // TODO: known_hosts Handling
            _ => {}
        }
        // Key/Passwort: für openssh via ssh-Optionen; Passwort interaktiv wird vermieden
        if let Some(auth) = &spec.auth
            && auth.kind == "key"
            && let Some(k) = &auth.key_path
        {
            let key = self.renderer.render_str(k, &self.ctx)?;
            ssh_cmd.extend(["-i", &key].iter().map(|s| s.to_string()));
        }
        ssh_cmd.push(format!("{}@{}", user, host));
        // ENV inline export
        let env_export = if env.is_empty() {
            "".to_string()
        } else {
            let assigns = env
                .iter()
                .map(|(k, v)| format!("{}={}", k, shell_escape::escape(v.into())))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{} ", assigns)
        };
        ssh_cmd.push(format!("{}{}", env_export, command));

        let line = ssh_cmd.join(" ");
        self.print_header(idx, step.name.as_deref().unwrap_or("ssh"), &line);

        if self.dry_run {
            return Ok(());
        }

        let mut child = Command::new(&ssh_cmd[0])
            .args(&ssh_cmd[1..])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("ssh spawn")?;

        self.stream_child(&mut child, "ssh").await
    }

    fn merge_env(
        &self,
        step_env: &std::collections::HashMap<String, String>,
        local_env: &std::collections::HashMap<String, String>,
    ) -> Result<std::collections::HashMap<String, String>> {
        let mut env = std::env::vars().collect::<std::collections::HashMap<_, _>>();
        for (k, v) in step_env {
            env.insert(k.clone(), self.renderer.render_str(v, &self.ctx)?);
        }
        for (k, v) in local_env {
            env.insert(k.clone(), self.renderer.render_str(v, &self.ctx)?);
        }
        Ok(env)
    }

    fn print_header(&self, idx: usize, kind: &str, rendered: &str) {
        println!("\n==[{}] {} ==", idx + 1, kind);
        println!("-> {}", rendered);
    }

    async fn stream_child(&self, child: &mut tokio::process::Child, prefix: &str) -> Result<()> {
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let mut out_reader = BufReader::new(stdout).lines();
        let mut err_reader = BufReader::new(stderr).lines();

        let prefix_owned = prefix.to_string();
        let prefix_owned2 = prefix.to_string();

        let out_task = tokio::spawn(async move {
            while let Ok(Some(line)) = out_reader.next_line().await {
                println!("[{}][out] {}", prefix_owned, line);
            }
        });
        let err_task = tokio::spawn(async move {
            while let Ok(Some(line)) = err_reader.next_line().await {
                eprintln!("[{}][err] {}", prefix_owned2, line);
            }
        });

        let status = child.wait().await?;
        let _ = tokio::join!(out_task, err_task);
        if !status.success() {
            anyhow::bail!("Prozess endete mit Status {}", status);
        }
        Ok(())
    }
}
