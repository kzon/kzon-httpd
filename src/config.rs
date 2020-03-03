use std::fs;

const CONFIG_FILE: &str = "/etc/httpd.conf";

pub struct Config {
    pub document_root: String,
}

pub fn get() -> Result<Config, std::io::Error> {
    let mut config = Config {
        document_root: String::from(""),
    };
    let config_str = fs::read_to_string(CONFIG_FILE)?;
    let lines: Vec<&str> = config_str.split("\n").collect();
    for line in lines.iter() {
        let parts: Vec<&str> = line.splitn(2, " ").collect();
        if parts.len() < 2 {
            continue;
        }
        let name = parts[0];
        let value = parts[1];
        match name {
            "document_root" => config.document_root = String::from(value),
            _ => (),
        }
    }

    Ok(config)
}
