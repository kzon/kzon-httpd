extern crate chrono;

use std::fs;
use std::io::prelude::*;
use std::path;

use chrono::Utc;

const SERVER_HEADER: &str = "kzon-httpd";
const CONNECTION_HEADER: &str = "keep-alive";

const DEFAULT_CONTENT_TYPE: &str = "text/plain";

pub fn write_status(status: i32) -> Vec<u8> {
    return write(status, &[], 0, DEFAULT_CONTENT_TYPE);
}

pub fn write_head(status: i32, content_len: usize, content_type: &str) -> Vec<u8> {
    return write(status, &[], content_len, content_type);
}

pub fn write_content(status: i32, content: &[u8], content_type: &str) -> Vec<u8> {
    return write(status, content, content.len(), content_type);
}

pub fn write(status: i32, body: &[u8], content_len: usize, content_type: &str) -> Vec<u8> {
    // println!("< {}", status);
    let headers = [
        format!("HTTP/1.1 {} {}", status, get_status_name(status)),
        format!("Server: {}", SERVER_HEADER),
        format!("Date: {}", Utc::now().to_rfc2822()),
        format!("Connection: {}", CONNECTION_HEADER),
        format!("Content-Type: {}", content_type.to_string()),
        format!("Content-Length: {}", content_len),
        "\r\n".to_string(),
    ];
    let mut response = headers.join("\r\n").to_string().into_bytes();
    response.extend(body);
    return response;
}

pub fn send_file(filepath: String, method: &str) -> Vec<u8> {
    let mut file: std::fs::File;
    match fs::File::open(&filepath) {
        Ok(f) => file = f,
        Err(_err) => {
            return write_status(404);
        }
    }

    let ext = match path::Path::new(&filepath).extension() {
        Some(e) => e.to_str().unwrap(),
        None => "",
    };
    let content_type = get_content_type(ext);

    let mut buf: Vec<u8> = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    if method == "HEAD" {
        return write_head(200, buf.len(), content_type);
    } else {
        return write_content(200, &buf[..], content_type);
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

fn get_status_name(status: i32) -> &'static str {
    match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not found",
        _ => "",
    }
}
