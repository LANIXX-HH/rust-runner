# Rust Runner - Verwendungsanleitung

## Übersicht

Rust Runner ist ein CLI-Tool, das YAML-Dateien einliest und die beschriebenen Schritte ausführt. Es unterstützt verschiedene Arten von Operationen:

- **shell**: Lokale Shell-Befehle
- **exec**: Lokale Binaries ohne Shell
- **ssh**: Remote-Befehle via SSH
- **conf**: Konfigurationsdateien mit Templates

## Installation

```bash
# Projekt bauen
cargo build --release

# Binary ist verfügbar unter:
./target/release/rust-runner
```

## Grundlegende Verwendung

```bash
# Normale Ausführung
./target/release/rust-runner playbook.yaml

# Dry-Run (nur Vorschau, keine Ausführung)
./target/release/rust-runner --dry-run playbook.yaml

# Verbose-Modus (mehr Ausgaben)
./target/release/rust-runner --verbose playbook.yaml
```

## YAML-Struktur

### Grundaufbau

```yaml
version: 1
globals:
  # Globale Variablen hier definieren
  app_name: myapp
  version: "1.0.0"
  
steps:
  - name: "Schritt-Name"
    # Einer der folgenden Blöcke:
    shell: { ... }
    # oder
    exec: { ... }
    # oder
    ssh: { ... }
    # oder
    conf: { ... }
```

### Shell-Befehle

```yaml
- name: Shell-Befehl ausführen
  shell:
    command: "echo 'Hello {{ app_name }}!'"
    env:
      CUSTOM_VAR: "wert"
    cwd: "/tmp"                    # optional
    shell: "bash -c"               # optional, default: "bash -c"
```

### Exec-Befehle (ohne Shell)

```yaml
- name: Binary ausführen
  exec:
    cmd: "ls"
    args:
      - "-la"
      - "{{ some_path }}"
    env:
      LS_COLORS: "auto"
    cwd: "."                       # optional
```

### SSH-Befehle

```yaml
- name: Remote-Befehl
  ssh:
    host: "{{ target_host }}"
    user: "{{ deploy_user }}"      # optional, default: "root"
    auth:                          # optional
      kind: "key"                  # "key" oder "password"
      key_path: "~/.ssh/id_rsa"    # für key auth
      # password: "secret"         # für password auth
    command: "systemctl status {{ app_name }}"
    env:
      REMOTE_VAR: "wert"
    check_host: "no"               # "yes"|"no"|"fingerprint", default: "no"
```

### Konfigurationsdateien

```yaml
- name: Config-Datei erstellen
  conf:
    dest: "/etc/{{ app_name }}/config.toml"
    template: |
      [app]
      name = "{{ app_name }}"
      version = "{{ version }}"
      user = "{{ ENV.USER }}"      # Zugriff auf Umgebungsvariablen
    backup: true                   # optional, erstellt .bak-Datei
    mode: "0644"                   # optional, Unix-Permissions
```

### Bedingte Ausführung

```yaml
- name: Bedingter Schritt
  when: true                       # oder false
  shell:
    command: "echo 'Wird nur ausgeführt wenn when: true'"
```

## Variablen-System

### Globale Variablen

```yaml
globals:
  app_name: myapp
  version: "1.0.0"
  database:
    host: localhost
    port: 5432
```

### Verwendung in Templates

```yaml
steps:
  - name: Verwende Variablen
    shell:
      command: "echo 'App: {{ app_name }}, Version: {{ version }}'"
      
  - name: Verschachtelte Variablen
    shell:
      command: "echo 'DB: {{ database.host }}:{{ database.port }}'"
```

### Umgebungsvariablen

```yaml
- name: Umgebungsvariablen verwenden
  shell:
    command: "echo 'User: {{ ENV.USER }}, Home: {{ ENV.HOME }}'"
```

## Beispiele

### Einfaches Beispiel

```yaml
version: 1
globals:
  app_name: myapp

steps:
  - name: Begrüßung
    shell:
      command: "echo 'Hello from {{ app_name }}!'"
  
  - name: Verzeichnis auflisten
    exec:
      cmd: "ls"
      args: ["-la", "."]
```

### Deployment-Beispiel

```yaml
version: 1
globals:
  app_name: myapp
  version: "1.0.0"
  target_host: production.example.com
  deploy_user: deploy

steps:
  - name: Service stoppen
    ssh:
      host: "{{ target_host }}"
      user: "{{ deploy_user }}"
      command: "sudo systemctl stop {{ app_name }}"
  
  - name: Binary hochladen
    exec:
      cmd: "rsync"
      args:
        - "-avz"
        - "./target/release/{{ app_name }}"
        - "{{ deploy_user }}@{{ target_host }}:/opt/{{ app_name }}/"
  
  - name: Konfiguration erstellen
    conf:
      dest: "/tmp/{{ app_name }}.conf"
      template: |
        app_name={{ app_name }}
        version={{ version }}
      backup: true
  
  - name: Service starten
    ssh:
      host: "{{ target_host }}"
      user: "{{ deploy_user }}"
      command: "sudo systemctl start {{ app_name }}"
```

## Ausgabe-Format

Das Tool zeigt für jeden Schritt:

1. **Header**: `==[Nummer] Name ==`
2. **Befehl**: `-> gerendeter_befehl`
3. **Live-Output**: `[typ][out/err] ausgabe`

Beispiel:
```
==[1] Test Shell-Befehl ==
-> echo 'Hello from myapp!'
[shell][out] Hello from myapp!

==[2] Test Exec ==
-> ls '-la .'
[exec][out] total 64
[exec][out] drwxr-xr-x  8 user  staff  256 Aug  7 22:00 .
...
```

## Fehlerbehandlung

- Bei Fehlern wird der Exit-Code des fehlgeschlagenen Befehls zurückgegeben
- Template-Fehler (fehlende Variablen) führen zu klaren Fehlermeldungen
- SSH-Verbindungsfehler werden entsprechend gemeldet

## Sicherheitshinweise

- SSH StrictHostKeyChecking ist standardmäßig deaktiviert
- Für Produktionsumgebungen SSH-Keys verwenden
- Sensible Daten über Umgebungsvariablen einbinden
- Dry-Run verwenden um Befehle vor Ausführung zu prüfen

## Erweiterte Features (geplant)

- `timeout` und `retry` für Schritte
- `includes` für modulare Playbooks
- Erweiterte `when`-Bedingungen
- Remote-Konfigurationsdateien via SSH/SFTP
