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
        t.add_raw_template("inline", s.as_ref())
            .context("add template")?;
        let cjson = serde_json::to_value(ctx)?;
        let mut c = tera::Context::from_value(cjson)?;
        // ENV verfügbar machen
        c.insert(
            "ENV",
            &std::env::vars().collect::<std::collections::HashMap<_, _>>(),
        );
        Ok(t.render("inline", &c)?)
    }

    pub fn render_map(
        &self,
        map: &std::collections::HashMap<String, String>,
        ctx: &Value,
    ) -> Result<std::collections::HashMap<String, String>> {
        let mut out = std::collections::HashMap::new();
        for (k, v) in map {
            out.insert(k.clone(), self.render_str(v, ctx)?);
        }
        Ok(out)
    }
}
