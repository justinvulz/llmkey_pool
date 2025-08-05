use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub server: ServerConfig,
    pub proxy: ProxyConfig,
}

#[derive(Deserialize, Debug)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3030
}

#[derive(Deserialize, Debug)]
pub struct ProxyConfig {
    #[serde(default)]
    pub proxy_api_key: String,
    pub gemini_api_keys: Vec<String>,
}

