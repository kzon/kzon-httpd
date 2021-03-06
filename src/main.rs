// ab -n 10000 -c 128 -k http://127.0.0.1:7878/httptest/dir1/dir12/dir123/deep.txt
// wrk -d 5s -t 4 -c 128 http://127.0.0.1:7878/httptest/dir1/dir12/dir123/deep.txt
// wrk -d 5s -t 4 -c 128 http://127.0.0.1:7878/httptest/wikipedia_russia.html

mod config;
mod http;

extern crate mio;
extern crate urldecode;

use mio::net::{TcpListener, TcpStream};
use mio::{Events, Poll, PollOpt, Ready, Token};
use std::collections::HashMap;
use std::io;
use std::io::{Read, Write};
use std::path::Path;
use std::thread;

fn main() -> Result<(), io::Error> {
    let config = config::get()?;

    println!("workers num: {}", config.workers_num);
    println!("document root: {}", config.document_root);

    let address = "0.0.0.0:7878";
    let listener = TcpListener::bind(&address.parse().unwrap()).unwrap();

    for n in 0..config.workers_num - 1 {
        let listener = listener.try_clone().unwrap();
        let config = config::Config{
            workers_num: config.workers_num,
            document_root: config.document_root.clone(),
        };
        thread::spawn(move || {
            serve(n + 1, listener, &config);
        });
    }

    serve(config.workers_num, listener, &config);

    Ok(())
}

struct Response {
    data: Vec<u8>,
    proto: http::Proto,
}

fn serve(n: usize, listener: TcpListener, config: &config::Config) {
    let poll = Poll::new().unwrap();
    poll.register(&listener, Token(0), Ready::readable(), PollOpt::edge())
        .unwrap();

    let mut counter: usize = 0;
    let mut sockets: HashMap<Token, TcpStream> = HashMap::new();
    let mut requests: HashMap<Token, Vec<u8>> = HashMap::new();
    let mut responses: HashMap<Token, Response> = HashMap::new();
    let mut buffer = [0 as u8; 1024];

    let mut events = Events::with_capacity(1024);
    loop {
        poll.poll(&mut events, None).unwrap();
        for event in &events {
            match event.token() {
                Token(0) => loop {
                    match listener.accept() {
                        Ok((socket, _)) => {
                            counter += 1;
                            let token = Token(counter);

                            poll.register(&socket, token, Ready::readable(), PollOpt::edge())
                                .unwrap();

                            sockets.insert(token, socket);
                            requests.insert(token, Vec::with_capacity(192));
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(_) => break,
                    }
                },
                token if event.readiness().is_readable() => {
                    loop {
                        let read = sockets.get_mut(&token).unwrap().read(&mut buffer);
                        match read {
                            Ok(0) => {
                                sockets.remove(&token);
                                break;
                            }
                            Ok(n) => {
                                let request = requests.get_mut(&token).unwrap();
                                for b in &buffer[0..n] {
                                    request.push(*b);
                                }
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                            Err(_) => break,
                        }
                    }

                    let ready = requests
                        .get(&token)
                        .unwrap()
                        .windows(4)
                        .find(|window| is_double_crnl(*window))
                        .is_some();

                    if ready {
                        if let Some(socket) = sockets.get(&token) {
                            io::stdout().flush().unwrap();
                            let request = requests.get_mut(&token).unwrap();
                            let response = handle_request(n, request, &config);
                            responses.insert(token, response);
                            request.clear();
                            poll.reregister(
                                socket,
                                token,
                                Ready::writable(),
                                PollOpt::edge() | PollOpt::oneshot(),
                            )
                            .unwrap();
                        }
                    }
                }
                token if event.readiness().is_writable() => {
                    let socket = sockets.get_mut(&token).unwrap();
                    let response = responses.get_mut(&token).unwrap();
                    let pending_write_buffer = &mut response.data;
                    while !pending_write_buffer.is_empty() {
                        match socket.write(pending_write_buffer) {
                            Ok(0) => break,
                            Ok(written) => {
                                let cut_len = pending_write_buffer.len() - written;
                                for n in 0 .. cut_len {
                                    pending_write_buffer[n] = pending_write_buffer[n + written];
                                }
                                pending_write_buffer.resize(cut_len, 0);
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {},
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                poll.reregister(
                                    socket,
                                    token,
                                    Ready::writable(),
                                    PollOpt::edge() | PollOpt::oneshot(),
                                )
                                .unwrap();
                                break
                            }
                            Err(_) => {
                                break
                            }
                        }
                    }
                    if pending_write_buffer.is_empty() {
                        match response.proto {
                            http::Proto::HTTP10 => socket.shutdown(std::net::Shutdown::Both).unwrap(),
                            http::Proto::HTTP11 => poll.reregister(
                                socket,
                                token,
                                Ready::readable(),
                                PollOpt::edge() | PollOpt::oneshot(),
                            )
                            .unwrap(),
                        }
                        responses.remove(&token);
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

fn is_double_crnl(window: &[u8]) -> bool {
    window.len() >= 4
        && (window[0] == '\r' as u8)
        && (window[1] == '\n' as u8)
        && (window[2] == '\r' as u8)
        && (window[3] == '\n' as u8)
}

fn handle_request(n: usize, req_bytes: &Vec<u8>, config: &config::Config) -> Response {
    let request = String::from_utf8_lossy(req_bytes);
    println!("#{} > {}\n", n, request);

    let parts: Vec<&str> = request.splitn(4, " ").collect();
    let method = parts[0];
    let path = parts[1];
    let mut proto = parts[2].to_string();
    proto.truncate(proto.len() - 4);  // remove newlines

    let data = if method != "GET" && method != "HEAD" {
        http::write_status(405)
    } else {
        send_file(&config.document_root, path.to_string(), method)
    };

    Response{
        data,
        proto: http::get_proto(proto.as_str()),
    }
}

fn send_file(document_root: &String, mut path: String, method: &str) -> Vec<u8> {
    if let Some(i) = path.find("?") {
        path.split_off(i);
    }

    path = urldecode::decode(path);

    path.insert_str(0, document_root);
    let mut path_meta = Path::new(&path);
    if !path_meta.exists() {
        return http::write_status(404);
    }
    path = String::from(path_meta.canonicalize().unwrap().to_str().unwrap());
    path_meta = Path::new(&path);
    if !path_meta.exists() || !path_meta.starts_with(document_root) {
        return http::write_status(404);
    }

    if path_meta.is_dir() {
        path.push_str("/index.html");
        path_meta = Path::new(&path);
        if !path_meta.exists() {
            return http::write_status(403);
        }
    }

    return http::send_file(path, method);
}
