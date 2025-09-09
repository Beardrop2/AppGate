use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Rule {
    pub name: String,
    pub protocol: String,        // "http" | "tcp" | "udp"
    pub resource: String,        // glob/prefix
    pub require_groups: Vec<String>,
    pub inject: Option<std::collections::HashMap<String,String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Policy {
    pub rules: Vec<Rule>,
}

impl Policy {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let txt = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&txt)?)
    }

    pub fn decide(
        &self,
        protocol: &str,
        resource: &str,
        groups: &[String],
    ) -> (bool, Option<std::collections::HashMap<String,String>>, String) {
        for r in &self.rules {
            if r.protocol == protocol && resource.starts_with(&r.resource) {
                let ok = r.require_groups.iter().all(|g| groups.contains(g));
                let reason = if ok { format!("policy: {}", r.name) } else { "missing group".into() };
                return (ok, r.inject.clone(), reason);
            }
        }
        (false, None, "default-deny".into())
    }
}