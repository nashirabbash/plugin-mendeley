use crate::logger::Logger;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::thread;
use std::time::Duration;
use tiny_http::{Header, Response, Server};

const HOST: &str = "127.0.0.1";
const PORT: u16 = 8080;
const SERVICE_NAME: &str = "mendeley-loopback";

fn health_payload(value: &Value) -> bool {
    value == &json!({"service": SERVICE_NAME, "status": "ok"})
}

fn request(host: &str, port: u16, method: &str, path: &str) -> std::io::Result<(u16, String)> {
    let address: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(
        format!("{method} {path} HTTP/1.0\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let (head, body) = response.split_once("\r\n\r\n").unwrap_or((&response, ""));
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse().ok())
        .unwrap_or(0);
    Ok((status, body.to_owned()))
}

fn running_service(host: &str, port: u16) -> bool {
    request(host, port, "GET", "/health")
        .ok()
        .is_some_and(|(status, body)| {
            status == 200 && serde_json::from_str(&body).is_ok_and(|value| health_payload(&value))
        })
}

pub fn stop(logger: &Logger) {
    if !running_service(HOST, PORT) {
        logger.event("info", "server.stop_not_running", json!({"port": PORT}));
        return;
    }
    match request(HOST, PORT, "POST", "/shutdown") {
        Ok((200, _)) => {
            for _ in 0..20 {
                if !running_service(HOST, PORT) {
                    logger.event("success", "server.stopped", json!({"port": PORT}));
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
            logger.event("error", "server.stop_timeout", json!({"port": PORT}));
        }
        Ok((status, _)) => logger.event(
            "error",
            "server.stop_failed",
            json!({"port": PORT, "status": status}),
        ),
        Err(error) => logger.event(
            "error",
            "server.stop_failed",
            json!({"port": PORT, "error": error.to_string()}),
        ),
    }
}

fn send_json(request: tiny_http::Request, status: u16, value: Value) {
    let body = value.to_string();
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(
            Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
        );
    let _ = request.respond(response);
}

pub fn run(logger: &Logger) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = match Server::http(format!("{HOST}:{PORT}")) {
        Ok(server) => server,
        Err(error) => {
            if running_service(HOST, PORT) {
                logger.event("info", "server.reused", json!({"host": HOST, "port": PORT}));
                return Ok(());
            }
            logger.event(
                "error",
                "server.start_failed",
                json!({"host": HOST, "port": PORT, "error": error.to_string()}),
            );
            return Err(error.into());
        }
    };
    logger.event(
        "success",
        "server.started",
        json!({"host": HOST, "port": PORT}),
    );
    let mut stopping = false;
    while !stopping {
        let Some(request) = server.recv_timeout(Duration::from_millis(100))? else {
            continue;
        };
        let method = request.method().as_str().to_owned();
        let path = request.url().split('?').next().unwrap_or("").to_owned();
        match (method.as_str(), path.as_str()) {
            ("GET", "/health") => send_json(
                request,
                200,
                json!({"service": SERVICE_NAME, "status": "ok"}),
            ),
            ("POST", "/shutdown") => {
                send_json(request, 200, json!({"status": "stopping"}));
                stopping = true;
                logger.event("info", "server.shutdown_requested", json!({"port": PORT}));
            }
            _ => send_json(request, 404, json!({"error": "not_found"})),
        }
    }
    logger.event("info", "server.stopped", json!({"port": PORT}));
    Ok(())
}
