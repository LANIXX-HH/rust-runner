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

```yaml
version: 1
globals:
  app_name: myapp
  user: deploy
  host: 1.2.3.4
  bin_path: "/opt/{{ app_name }}/bin/{{ app_name }}"
  env:
    RUST_LOG: info

steps:
  - name: Shell-Befehl
    shell:
      command: "sudo systemctl stop {{ app_name }}"
      env:
        CUSTOM_VAR: "value"
      cwd: "/tmp"
      shell: "bash -c"  # optional, default: "bash -c"
  
  - name: Binary ausführen
    exec:
      cmd: rsync
      args:
        - "-avz"
        - "./target/release/{{ app_name }}"
        - "{{ user }}@{{ host }}:{{ bin_path }}"
      env:
        RSYNC_OPTS: "--progress"
      cwd: "."  # optional
  
  - name: SSH-Befehl
    ssh:
      host: "{{ host }}"
      user: "{{ user }}"
      auth:
        kind: key  # oder "password"
        key_path: "~/.ssh/id_rsa"
        # password: "secret"  # für password auth
      command: "{{ bin_path }} migrate --yes"
      env:
        DATABASE_URL: "postgres://..."
      check_host: "no"   # "yes"|"no"|"fingerprint"
  
  - name: Konfigurationsdatei
    conf:
      dest: "/etc/{{ app_name }}/app.toml"
      template: |
        app = "{{ app_name }}"
        log = "info"
        user = "{{ ENV.USER }}"  # Zugriff auf Umgebungsvariablen
      backup: true  # optional, erstellt .bak-Datei
      mode: "0644"  # optional, Unix-Permissions
```

## Variablen-System

- **Globale Variablen**: Definiert im `globals`-Abschnitt
- **Umgebungsvariablen**: Zugriff über `{{ ENV.VARIABLE_NAME }}`
- **Template-Engine**: Tera-Syntax mit sicherer Variablenersetzung
- **Verschachtelte Objekte**: Unterstützung für komplexe Datenstrukturen

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
