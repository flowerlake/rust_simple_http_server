extern crate regex;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fs::{DirEntry, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::{env, fs};
use urlencoding::decode;

use https_rs::ThreadPool;
use regex::Regex;

lazy_static! {
    static ref EXT_CONTENT_TYPE: HashMap<&'static str, &'static str> = {
        let mime_types: HashMap<&str, &str> = vec![
            ("html", "text/html"),
            ("htm", "text/html"),
            ("xhtml", "text/html"),
            ("css", "text/css"),
            ("js", "text/javascript"),
            ("jpg", "image/jpeg"),
            ("jpeg", "image/jpeg"),
            ("png", "image/png"),
            ("ico", "image/x-icon"),
            ("svg", "image/svg+xml"),
            ("gif", "image/gif"),
            ("avif", "image/avif"),
            ("webp", "image/webp"),
            ("pdf", "application/pdf"),
            ("json", "application/json"),
            ("mp4", "video/mp4"),
            ("mp3", "video/mp3"),
            ("txt", "text/plain"),
        ]
        .into_iter()
        .collect();
        mime_types
    };
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let listen_addr: &str;
    if args.len() == 2 {
        listen_addr = &args[1];
    } else {
        listen_addr = "0.0.0.0:7878";
    }
    println!(
        "Serving HTTP on {} (http://{}/) ...",
        listen_addr, listen_addr
    );
    let listener = TcpListener::bind(listen_addr).unwrap();
    let thread_pool = ThreadPool::new(8);
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();

        thread_pool.execute(move || {
            print!("[{:?}] ", stream.peer_addr().unwrap());
            let request_header = read_stream(&stream);
            match request_header {
                None => {}
                Some(y) => {
                    create_resp(y.clone(), &mut stream);
                }
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
    let regex = Regex::new(r"(?m)GET ([/a-zA-Z0-9%.].*) HTTP/1.1").unwrap();
    // result will be an iterator over tuples containing the start and end indices for each match in the string
    let mut result = regex.captures_iter(http_header);

    if let Some(mat) = result.next() {
        mat.get(1).map_or(None, |m| {Some(decode_url(m.as_str()))})
    } else {
        None
    }
}

fn decode_url(encoded_str: &str) -> String{
    let decoded_str = decode(encoded_str).expect("UTF-8");
    decoded_str
}

fn create_resp(request_line: String, stream: &mut TcpStream) {
    println!("{}", request_line);
    let path = match_url_path(&request_line);
    match path {
        None => {
            let status_line = "HTTP/1.1 404 NOT Found";
            let contents = format!("HTTP/1.1 404 NOT Found");
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
    let http_req: Vec<String> = buf
        .lines()
        .map(|line| line.unwrap())
        .take_while(|line| !line.is_empty())
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
    let path = Path::new(filepath);
    let file = File::open(path);
    match file {
        Ok(mut x) => {
            let status_line = "HTTP/1.1 200 OK";
            let filename = path.file_name().unwrap().to_str().unwrap();
            let file_type = path.extension();
            let content_type;
            match file_type {
                Some(x) => match EXT_CONTENT_TYPE.get(x.to_str().unwrap()) {
                    Some(x) => {
                        content_type = *x;
                    }
                    None => {
                        content_type = "application/octet-stream";
                    }
                },
                None => {
                    content_type = "application/octet-stream";
                }
            }

            let length = x.metadata().unwrap().len();
            let response_headers = format!("{status_line}\r\nContent-type: {content_type}\r\nContent-Disposition: attachment; filename={filename}\r\nContent-Length: {length}\r\n\r\n");
            let _ = stream.write(response_headers.as_bytes()).unwrap();

            let mut buffer = [0u8; 1024];
            while let Ok(size) = x.read(&mut buffer) {
                if size == 0 {
                    break;
                }
                let _ = stream.write(buffer.as_ref());
            }
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
    use urlencoding::decode;

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
                }
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
            Some(x) => {
                println!("{}", x);
            }
        }
    }

    #[test]
    fn urldecode_test(){
        let res = decode("/HCIP-Security%20V4.0%20%E5%AE%9E%E9%AA%8C%E6%89%8B%E5%86%8C.pdf").unwrap();
        assert_eq!(res, "/HCIP-Security V4.0 实验手册.pdf");
    }
}
