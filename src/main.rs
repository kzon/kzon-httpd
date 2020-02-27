extern crate chrono;

use chrono::Utc;
use std::fs;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;

const CONFIG_FILE: &str = "/etc/httpd.conf";

const SERVER_HEADER: &str = "kzon-httpd";
const CONNECTION_HEADER: &str = "close";

const DEFAULT_CONTENT_TYPE: &str = "text/plain";

struct Config {
    cpu_limit: i32,
    document_root: String,
}

fn main() -> Result<(), std::io::Error> {
    let config = read_config()?;

    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        handle_connection(stream, &config);
    }

    Ok(())
}

fn read_config() -> Result<Config, std::io::Error> {
    let mut config = Config {
        cpu_limit: 0,
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
            "cpu_limit" => config.cpu_limit = value.parse().unwrap(),
            "document_root" => config.document_root = String::from(value),
            _ => (),
        }
    }

    Ok(config)
}

fn handle_connection(mut stream: TcpStream, config: &Config) {
    let mut buffer = [0; 512];
    stream.read(&mut buffer).unwrap();

    let request = String::from_utf8_lossy(&buffer[..]);
    let parts: Vec<&str> = request.splitn(3, " ").collect();
    let method = parts[0];

    if method != "GET" && method != "HEAD" {
        http_write(stream, 405, String::from(""));
    } else {
        let path = parts[1];
        send_file(stream, format!("{}{}", config.document_root, path));
    }
}

fn http_write(stream: TcpStream, status: i32, content: String) {
    http_write_with_content_type(stream, status, content, DEFAULT_CONTENT_TYPE.to_string())
}

fn http_write_with_content_type(mut stream: TcpStream, status: i32, content: String, content_type: String) {
    let date = Utc::now().to_rfc2822();
    let headers = format!(
        "Server: {}\r\nDate: {}\r\nConnection: {}\r\nContent-Type: {}",
        SERVER_HEADER, date, CONNECTION_HEADER, content_type,
    );
    let response = format!("HTTP/1.1 {}\r\n{}\r\n\r\n{}", status, headers, content);
    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}

fn send_file(stream: TcpStream, filepath: String) {
    match fs::read_to_string(&filepath) {
        Ok(content) => {
            let ext = std::path::Path::new(&filepath).extension();
            let content_type = match ext {
                None => DEFAULT_CONTENT_TYPE,
                Some(ext) => get_content_type(ext.to_str().unwrap()),
            };
            http_write_with_content_type(stream, 200, content, content_type.to_string());
        },
        Err(err) => http_write(stream, 404, err.to_string()),
    }
}

fn get_content_type(ext: &str) -> &str {
    match ext {
        "html" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "swf" => "application/x-shockwave-flash",
        _ => DEFAULT_CONTENT_TYPE,
    }
}
