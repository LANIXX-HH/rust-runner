// src/schema.rs
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct Document {
    #[allow(dead_code)]
    pub version: u32,
    #[serde(default)]
    pub globals: serde_yaml::Value,
    pub steps: Vec<Step>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub struct SshAuth {
    pub kind: String, // "password" | "key"
    #[allow(dead_code)]
    pub password: Option<String>, // templated
    pub key_path: Option<String>, // templated
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub timeout: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
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
