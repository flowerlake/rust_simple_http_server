extern crate regex;
use std::fs;
use std::fs::{DirEntry, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

use hello::ThreadPool;
use regex::Regex;

fn main() {
    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    let thread_pool = ThreadPool::new(8);
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();

        thread_pool.execute(move || {
            print!("Handling connection from {:?}\n", stream.peer_addr().unwrap());
            let request_header = read_stream(&stream);
            match request_header {
                None => {}
                Some(y) => { create_resp(y.clone(), &mut stream); }
            }
        });
    }
}


fn create_list_dir_resp(current_path: String, stream: &mut TcpStream) {
    let status_line = "HTTP/1.1 200 OK";
    let files = get_current_directory(&current_path);
    let mut body = String::new();
    body.push_str("<html lang=\"en\"><head><meta charset=\"utf-8\"><title>Directory listing for /</title></head><body><h1>Directory listing for /</h1><hr><ul>");

    for file_entry in files {
        body.push_str("<li><a href='");
        let filename = file_entry.file_name().to_str().unwrap().to_string();
        if file_entry.path().is_dir() {
            body.push_str(&filename);
            body.push_str("/'>");
            body.push_str(&filename);
            body.push_str("/</a></li>");
        } else if file_entry.file_type().unwrap().is_file() {
            body.push_str(&filename);
            body.push_str("'>");
            body.push_str(&filename);
            body.push_str("</a></li>");
        }
    }
    body.push_str("</ul><hr></body></html>");
    let contents = body;
    let length = contents.len();
    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

    stream.write(response.as_bytes()).unwrap();
}

fn match_url_path(http_header: &str) -> Option<String> {
    let regex = Regex::new(r"(?m)GET ([/a-zA-Z0-9].*) HTTP/1.1").unwrap();
    // result will be an iterator over tuples containing the start and end indices for each match in the string
    let mut result = regex.captures_iter(http_header);

    if let Some(mat) = result.next() {
        mat.get(1).map_or(None, |m| Some(m.as_str().to_string()))
    } else { None }
}

fn create_resp(request_line: String, stream: &mut TcpStream) {
    let client_addr = stream.peer_addr().unwrap();
    println!("{}", request_line);
    let path = match_url_path(&request_line);
    match path {
        None => {
            let status_line = "HTTP/1.1 404 NOT Found";
            let contents = format!("HTTP/1.1 404 NOT Found {client_addr}");
            let length = contents.len();
            let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");
            stream.write(response.as_bytes()).unwrap();
        }
        Some(real_path) => {
            if real_path == "/" {
                let current_path = String::from(".");
                create_list_dir_resp(current_path, stream);
            } else {
                let url_path = String::from(real_path.strip_prefix("/").unwrap());
                let path = Path::new(&url_path);
                if path.is_file() {
                    download_file_response(&url_path, stream);
                }
                if path.is_dir() {
                    create_list_dir_resp(url_path, stream);
                }
            }
        }
    }
}

fn read_stream(stream: &TcpStream) -> Option<String> {
    let buf = BufReader::new(stream);
    let http_req: Vec<String> = buf.lines()
        .map(|line| { line.unwrap() })
        .take_while(|line| { !line.is_empty() })
        .collect();

    let line: Option<&String> = http_req.iter().next();
    let opt_string: Option<String> = line.map(|s| s.clone());
    opt_string
}

fn get_current_directory(current_path: &String) -> Vec<DirEntry> {
    let mut files: Vec<DirEntry> = Vec::new();
    for entry in fs::read_dir(current_path).unwrap() {
        files.push(entry.unwrap());
    }
    files
}

fn download_file_response(filepath: &String, stream: &mut TcpStream) {
    let file = File::open(Path::new(filepath));
    match file {
        Ok(mut x) => {
            let status_line = "HTTP/1.1 200 OK";
            let filename = Path::new(filepath).file_name().unwrap().to_str().unwrap();
            let length = x.metadata().unwrap().len();
            let response_headers = format!("{status_line}\r\nContent-type: application/octet-stream\r\nContent-Disposition: attachment; filename={filename}\r\nContent-Length: {length}\r\n\r\n");
            let _ = stream.write(response_headers.as_bytes()).unwrap();

            let mut buffer = [0u8; 1024];
            while let Ok(size) = x.read(&mut buffer) {
                if size == 0 {
                    break;
                }
                let _ = stream.write(buffer.as_ref());
            };
        }
        Err(_) => {
            let status_line = "HTTP/1.1 404 Not Found";
            let _response_headers = format!("{status_line}\r\n\r\n", status_line = status_line);
            let _ = stream.write_all(_response_headers.as_bytes()).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::match_url_path;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::path::Path;

    fn write_to_file() {
        let filepath = Path::new("LeiGodSetup.10.1.8.1.exe");
        let new_filepath = Path::new("Hitest2.exe");
        let file = File::open(filepath);
        let mut new_file = File::create(new_filepath).unwrap();
        match file {
            Ok(mut x) => {
                let mut buffer = [0u8; 1024];
                while let Ok(size) = x.read(&mut buffer) {
                    if size == 0 {
                        break;
                    }
                    let _ = new_file.write(buffer.as_ref());
                };
            }
            Err(_) => {
                let status_line = "HTTP/1.1 404 Not Found";
                let response_headers = format!("{status_line}\r\n\r\n", status_line = status_line);
                eprintln!("{response_headers}");
            }
        }
    }

    #[test]
    fn test() {
        write_to_file();
    }

    #[test]
    fn test2() {
        let string = "GET /sleep HTTP/1.1";
        let data = match_url_path(string);
        match data {
            None => {}
            Some(x) => { println!("{}", x); }
        }
    }
}
