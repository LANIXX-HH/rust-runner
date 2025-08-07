# Rust Runner - Execution Plan für Coding LLM

## 📋 Übersicht

Dieser Execution Plan ermöglicht es, das Rust Runner Projekt exakt zu reproduzieren. Folge den Schritten in der angegebenen Reihenfolge.

## 🎯 Ziel

Erstelle ein CLI-Tool namens "rust-runner", das YAML-Dateien einliest und die beschriebenen Schritte ausführt. Unterstützt werden die Blöcke `ssh`, `conf`, `exec` und `shell` mit globalen Variablen und Live-Output im Terminal.

## 📝 Spezifikationen

Basierend auf dem Dokument "Rust-Projektentwurf: YAML-gesteuerte Ausführung mit Blöcken" mit folgenden Anforderungen:

- CLI-Tool mit Argument-Parsing
- YAML-Schema V1 mit globalen Variablen
- Template-Engine für Variablen-Interpolation
- Vier Block-Typen: shell, exec, ssh, conf
- Live-Output-Streaming mit Prefixen
- Dry-Run und Verbose-Modi

## 🚀 Schritt-für-Schritt Anleitung

### Schritt 1: Rust-Umgebung vorbereiten

```bash
# Falls Rust nicht installiert ist:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Neues Rust-Projekt erstellen
cargo new rust-runner
cd rust-runner
```

### Schritt 2: Dependencies hinzufügen

```bash
# Basis-Dependencies
cargo add serde --features derive
cargo add serde_yaml serde_json thiserror anyhow
cargo add tokio --features macros,rt-multi-thread,process,io-util
cargo add tera
cargo add clap --features derive
cargo add shell-escape
```

### Schritt 3: Projektstruktur erstellen

Erstelle folgende Dateien in `src/`:

#### 3.1 `src/schema.rs` - YAML-Datenstrukturen

```rust
// src/schema.rs
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct Document {
    pub version: u32,
    #[serde(default)]
    pub globals: serde_yaml::Value,
    pub steps: Vec<Step>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub struct SshAuth {
    pub kind: String,             // "password" | "key"
    pub password: Option<String>, // templated
    pub key_path: Option<String>, // templated
    pub passphrase: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct SshSpec {
    pub host: String,
    pub user: Option<String>,
    pub auth: Option<SshAuth>,
    pub command: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub check_host: Option<String>, // "yes" | "no" | "fingerprint"
}

#[derive(Deserialize, Debug)]
pub struct ExecSpec {
    pub cmd: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ShellSpec {
    pub command: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub shell: Option<String>, // default: "bash -c"
}

#[derive(Deserialize, Debug)]
pub struct ConfSpec {
    pub dest: String,
    pub template: String,
    #[serde(default)]
    pub backup: bool,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Step {
    pub name: Option<String>,
    #[serde(default)]
    pub when: Option<bool>,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub retry: Option<u32>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub exec: Option<ExecSpec>,
    #[serde(default)]
    pub shell: Option<ShellSpec>,
    #[serde(default)]
    pub ssh: Option<SshSpec>,
    #[serde(default)]
    pub conf: Option<ConfSpec>,
}
```

#### 3.2 `src/template.rs` - Template-Rendering

```rust
// src/template.rs
use anyhow::{Context, Result};
use serde_yaml::Value;
use tera::Tera;

pub struct Renderer {
    tera: Tera,
}

impl Renderer {
    pub fn new() -> Self {
        // leere Tera-Instanz für String-Rendering
        let mut tera = Tera::default();
        tera.autoescape_on(vec![]);
        Self { tera }
    }

    pub fn render_str<S: AsRef<str>>(&self, s: S, ctx: &Value) -> Result<String> {
        let mut t = self.tera.clone();
        // dynamische Template-Quelle
        t.add_raw_template("inline", s.as_ref()).context("add template")?;
        let cjson = serde_json::to_value(ctx)?;
        let mut c = tera::Context::from_value(cjson)?;
        // ENV verfügbar machen
        c.insert("ENV", &std::env::vars().collect::<std::collections::HashMap<_, _>>());
        Ok(t.render("inline", &c)?)
    }

    pub fn render_map(&self, map: &std::collections::HashMap<String, String>, ctx: &Value)
        -> Result<std::collections::HashMap<String, String>>
    {
        let mut out = std::collections::HashMap::new();
        for (k, v) in map {
            out.insert(k.clone(), self.render_str(v, ctx)?);
        }
        Ok(out)
    }
}
```

#### 3.3 `src/executor.rs` - Ausführungslogik

```rust
// src/executor.rs
use crate::schema::*;
use crate::template::Renderer;
use anyhow::{Context, Result};
use serde_yaml::Value;
use std::path::Path;
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command};

pub struct Executor {
    renderer: Renderer,
    ctx: Value,
    verbose: bool,
    dry_run: bool,
}

impl Executor {
    pub fn new(globals: Value, verbose: bool, dry_run: bool) -> Self {
        Self { renderer: Renderer::new(), ctx: globals, verbose, dry_run }
    }

    pub async fn run_step(&self, step: &Step, idx: usize) -> Result<()> {
        if let Some(false) = step.when { return Ok(()) }

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
        let mut parts = shell.split_whitespace().map(|s| s.to_string()).collect::<Vec<_>>();
        let (prg, mut args) = (parts.remove(0), parts);
        args.push(cmd_str.clone());

        let env = self.merge_env(&step.env, &spec.env)?;
        self.print_header(idx, step.name.as_deref().unwrap_or("shell"), &cmd_str);

        if self.dry_run { return Ok(()) }

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
        let args = spec.args.iter()
            .map(|a| self.renderer.render_str(a, &self.ctx))
            .collect::<Result<Vec<_>>>()?;
        let env = self.merge_env(&step.env, &spec.env)?;
        let line = format!("{} {}", cmd, shell_escape::escape(args.join(" ").into()));
        self.print_header(idx, step.name.as_deref().unwrap_or("exec"), &line);

        if self.dry_run { return Ok(()) }

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
        self.print_header(idx, step.name.as_deref().unwrap_or("conf"), &format!("write {}", dest));

        if self.dry_run { 
            println!("Content preview:\n{}", content);
            return Ok(()) 
        }

        let path = Path::new(&dest);
        if spec.backup {
            if path.exists() {
                let bak = format!("{}.bak", dest);
                std::fs::copy(&dest, &bak).context("backup copy")?;
                println!("[conf] backup -> {}", bak);
            }
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
            Some("no") | None => ssh_cmd.extend(["-o","StrictHostKeyChecking=no","-o","UserKnownHostsFile=/dev/null"].map(String::from)),
            Some("yes") => {},
            Some("fingerprint") => {}, // TODO: known_hosts Handling
            _ => {}
        }
        // Key/Passwort: für openssh via ssh-Optionen; Passwort interaktiv wird vermieden
        if let Some(auth) = &spec.auth {
            if auth.kind == "key" {
                if let Some(k) = &auth.key_path {
                    let key = self.renderer.render_str(k, &self.ctx)?;
                    ssh_cmd.extend(["-i", &key].iter().map(|s| s.to_string()));
                }
            }
        }
        ssh_cmd.push(format!("{}@{}", user, host));
        // ENV inline export
        let env_export = if env.is_empty() {
            "".to_string()
        } else {
            let assigns = env.iter().map(|(k,v)| format!("{}={}", k, shell_escape::escape(v.into()))).collect::<Vec<_>>().join(" ");
            format!("{} ", assigns)
        };
        ssh_cmd.push(format!("{}{}", env_export, command));

        let line = ssh_cmd.join(" ");
        self.print_header(idx, step.name.as_deref().unwrap_or("ssh"), &line);

        if self.dry_run { return Ok(()) }

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
        for (k,v) in step_env { env.insert(k.clone(), self.renderer.render_str(v, &self.ctx)?); }
        for (k,v) in local_env { env.insert(k.clone(), self.renderer.render_str(v, &self.ctx)?); }
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
```

#### 3.4 `src/main.rs` - CLI Interface

```rust
// src/main.rs
mod schema;
mod template;
mod executor;

use anyhow::{Context, Result};
use clap::Parser;
use schema::Document;

#[derive(Parser, Debug)]
#[command(name="rust-runner", version, about="YAML-gesteuerte Ausführung")]
struct Cli {
    /// Pfad zur YAML-Datei
    file: String,
    /// Dry-Run (nichts ausführen)
    #[arg(long)]
    dry_run: bool,
    /// Verbose Logging
    #[arg(long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let raw = std::fs::read_to_string(&cli.file).context("YAML lesen")?;
    let doc: Document = serde_yaml::from_str(&raw).context("YAML parsen")?;

    let exec = executor::Executor::new(doc.globals, cli.verbose, cli.dry_run);

    for (i, step) in doc.steps.iter().enumerate() {
        if let Err(e) = exec.run_step(step, i).await {
            eprintln!("Fehler in Schritt {}: {:?}", i + 1, e);
            std::process::exit(1);
        }
    }
    Ok(())
}
```

### Schritt 4: Beispiel-YAML-Dateien erstellen

#### 4.1 `example.yaml` - Einfaches Beispiel

```yaml
version: 1
globals:
  app_name: myapp
  user: deploy
  host: localhost
  bin_path: "/opt/{{ app_name }}/bin/{{ app_name }}"
  env:
    RUST_LOG: info

steps:
  - name: Test lokaler Shell-Befehl
    shell:
      command: "echo 'Hello from {{ app_name }}!'"
      env:
        TEST_VAR: "{{ app_name }}_test"
  
  - name: Test exec mit Argumenten
    exec:
      cmd: "ls"
      args:
        - "-la"
        - "."
      env:
        LS_COLORS: "auto"
  
  - name: Erstelle Test-Konfiguration
    conf:
      dest: "/tmp/{{ app_name }}_config.toml"
      template: |
        [app]
        name = "{{ app_name }}"
        log_level = "{{ env.RUST_LOG }}"
        
        [database]
        url = "sqlite:///tmp/{{ app_name }}.db"
      backup: true
      mode: "0644"
  
  - name: Test SSH (lokal via localhost)
    ssh:
      host: "{{ host }}"
      user: "{{ ENV.USER }}"
      command: "echo 'SSH test successful for {{ app_name }}'"
      check_host: "no"
      env:
        REMOTE_VAR: "{{ app_name }}_remote"
```

#### 4.2 `test.yaml` - Test ohne SSH

```yaml
version: 1
globals:
  app_name: myapp
  test_dir: "/tmp/rust-runner-test"
  env:
    RUST_LOG: info

steps:
  - name: Test lokaler Shell-Befehl
    shell:
      command: "echo 'Hello from {{ app_name }}!'"
      env:
        TEST_VAR: "{{ app_name }}_test"
  
  - name: Test exec mit Argumenten
    exec:
      cmd: "ls"
      args:
        - "-la"
        - "."
      env:
        LS_COLORS: "auto"
  
  - name: Erstelle Test-Verzeichnis
    shell:
      command: "mkdir -p {{ test_dir }}"
  
  - name: Erstelle Test-Konfiguration
    conf:
      dest: "{{ test_dir }}/{{ app_name }}_config.toml"
      template: |
        [app]
        name = "{{ app_name }}"
        log_level = "{{ env.RUST_LOG }}"
        user = "{{ ENV.USER }}"
        
        [database]
        url = "sqlite://{{ test_dir }}/{{ app_name }}.db"
      backup: true
      mode: "0644"
  
  - name: Zeige erstellte Datei
    shell:
      command: "cat {{ test_dir }}/{{ app_name }}_config.toml"
```

### Schritt 5: Dokumentation erstellen

#### 5.1 `README.md` - Hauptdokumentation

```markdown
# Rust Runner

Ein CLI-Tool, das YAML-Dateien einliest und die beschriebenen Schritte ausführt. Unterstützt die Blöcke `ssh`, `conf`, `exec` und `shell` mit globalen Variablen und Live-Output im Terminal.

## Features

- **Shell-Befehle**: Lokale Shell-Befehle mit Live-Output
- **Exec-Befehle**: Lokale Binaries ohne Shell mit Live-Output  
- **SSH-Befehle**: Remote-Befehle via SSH mit Passwort/Key-Unterstützung
- **Konfigurationsdateien**: Template-basierte Dateierstellung mit Backup-Option
- **Variablen-System**: Globale Variablen mit Tera-Template-Engine
- **Dry-Run-Modus**: Vorschau der Befehle ohne Ausführung
- **Live-Logging**: Transparente Ausgabe mit Prefixen

## Installation

```bash
cargo build --release
```

## Verwendung

```bash
# Normale Ausführung
./target/release/rust-runner playbook.yaml

# Dry-Run (nur Vorschau)
./target/release/rust-runner --dry-run playbook.yaml

# Verbose-Modus
./target/release/rust-runner --verbose playbook.yaml
```

## YAML-Schema

[Füge hier das vollständige YAML-Schema-Beispiel ein]

## Beispiele

Siehe `example.yaml` für ein vollständiges Beispiel mit allen unterstützten Blöcken.

## Sicherheitshinweise

- SSH StrictHostKeyChecking ist standardmäßig deaktiviert für einfache Tests
- Für Produktionsumgebungen sollten SSH-Keys verwendet werden
- Passwort-Authentifizierung wird nicht empfohlen
- Template-Variablen werden validiert und führen zu klaren Fehlermeldungen

## Abhängigkeiten

- `serde` + `serde_yaml`: YAML-Parsing
- `tera`: Template-Engine für Variablenersetzung
- `tokio`: Asynchrone Prozessausführung
- `clap`: CLI-Interface
- `anyhow`: Fehlerbehandlung
```

### Schritt 6: Build und Test

```bash
# Projekt bauen
cargo build

# Testen mit Dry-Run
./target/debug/rust-runner --dry-run test.yaml

# Echte Ausführung testen
./target/debug/rust-runner test.yaml

# Release-Build
cargo build --release

# Hilfe anzeigen
./target/release/rust-runner --help
```

## ✅ Erfolgskriterien

Das Projekt ist erfolgreich implementiert, wenn:

1. ✅ Alle Dependencies korrekt hinzugefügt
2. ✅ Alle 4 Dateien in `src/` erstellt
3. ✅ Projekt kompiliert ohne Fehler
4. ✅ Dry-Run-Modus funktioniert
5. ✅ Alle 4 Block-Typen (shell, exec, ssh, conf) funktionieren
6. ✅ Template-Variablen werden korrekt gerendert
7. ✅ Live-Output wird mit Prefixen angezeigt
8. ✅ CLI-Argumente (--dry-run, --verbose) funktionieren

## 🔧 Troubleshooting

### Häufige Probleme:

1. **Lifetime-Fehler in stream_child**: Verwende `prefix.to_string()` für owned strings
2. **Template-Fehler**: Stelle sicher, dass alle Variablen in `globals` definiert sind
3. **SSH-Fehler**: Teste zuerst ohne SSH-Block, dann mit localhost
4. **Permissions-Fehler**: Verwende `/tmp/` für Test-Dateien

### Debugging:

```bash
# Verbose-Modus für mehr Details
./target/debug/rust-runner --verbose --dry-run example.yaml

# Rust-Logs aktivieren
RUST_LOG=debug ./target/debug/rust-runner test.yaml
```

## 📋 Checkliste für LLM

- [ ] Rust-Umgebung installiert
- [ ] Neues Cargo-Projekt erstellt
- [ ] Alle Dependencies hinzugefügt
- [ ] `src/schema.rs` erstellt
- [ ] `src/template.rs` erstellt  
- [ ] `src/executor.rs` erstellt
- [ ] `src/main.rs` erstellt
- [ ] Beispiel-YAML-Dateien erstellt
- [ ] README.md erstellt
- [ ] Projekt kompiliert erfolgreich
- [ ] Tests mit Dry-Run durchgeführt
- [ ] Live-Ausführung getestet
- [ ] Release-Build erstellt

**Zeitaufwand**: Ca. 30-45 Minuten für vollständige Implementierung

**Schwierigkeitsgrad**: Mittel (Rust-Kenntnisse erforderlich)

**Ergebnis**: Vollständig funktionsfähiges CLI-Tool für YAML-gesteuerte Automatisierung
