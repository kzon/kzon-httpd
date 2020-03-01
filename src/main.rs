mod http;

use std::fs;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;

extern crate urldecode;

const CONFIG_FILE: &str = "/etc/httpd.conf";

struct Config {
    cpu_limit: i32,
    document_root: String,
}

fn main() -> Result<(), std::io::Error> {
    let config = read_config()?;

    println!("cpu limit: {}", config.cpu_limit);
    println!("document root: {}", config.document_root);

    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    println!("server is listening");
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
    println!("> {}", request);
    if request.len() == 0 {
        return;
    }

    let parts: Vec<&str> = request.splitn(3, " ").collect();
    let method = parts[0];
    if method != "GET" && method != "HEAD" {
        http::write_status(stream, 405);
    } else {
        let path = parts[1];
        send_file(stream, &config.document_root, path.to_string(), method);
    }

    println!();
}

fn send_file(stream: TcpStream, document_root: &String, mut path: String, method: &str) {
    if let Some(i) = path.find("?") {
        path.split_off(i);
    }

    path = urldecode::decode(path);

    path.insert_str(0, document_root);
    let mut path_meta = Path::new(&path);
    if !path_meta.exists() {
        http::write_status(stream, 404);
        return;
    }
    path = String::from(path_meta.canonicalize().unwrap().to_str().unwrap());
    path_meta = Path::new(&path);
    if !path_meta.exists() || !path_meta.starts_with(document_root) {
        http::write_status(stream, 404);
        return;
    }

    if path_meta.is_dir() {
        path.push_str("/index.html");
        path_meta = Path::new(&path);
        if !path_meta.exists() {
            http::write_status(stream, 403);
            return;
        }
    }

    http::send_file(stream, path, method);
}
