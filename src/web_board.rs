use crate::{
    domain::{TaskRecord, TaskStatus},
    output,
    store::TaskStore,
};
use anyhow::{bail, Context, Result};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};

pub const DEFAULT_REFRESH_SECONDS: u64 = 5;

pub fn render_task_board_html(tasks: &[TaskRecord]) -> String {
    render_task_board_html_with_refresh(tasks, DEFAULT_REFRESH_SECONDS)
}

pub fn render_task_board_html_with_refresh(tasks: &[TaskRecord], refresh_seconds: u64) -> String {
    let board = output::task_board(tasks);
    let escaped_board = escape_html(&board);

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="refresh" content="{refresh_seconds}">
<title>Task Board</title>
<style>
body {{ margin: 2rem; font-family: system-ui, sans-serif; }}
pre {{ white-space: pre-wrap; word-break: break-word; }}
</style>
</head>
<body>
<main>
<h1>Task Board</h1>
<p>No write actions are available in this read-only view.</p>
<pre>{escaped_board}</pre>
</main>
</body>
</html>
"#
    )
}

pub fn load_task_board_tasks(store: &TaskStore) -> Result<Vec<TaskRecord>> {
    let mut tasks = store.list_tasks()?;
    tasks.retain(|task| task.status != TaskStatus::Archived);
    tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(tasks)
}

pub fn serve_task_board(store: &TaskStore, host: &str, port: u16) -> Result<()> {
    let bind_address = loopback_bind_address(host, port)?;
    let listener = TcpListener::bind(bind_address)
        .with_context(|| format!("bind HelmAgent board server on {host}:{port}"))?;
    let local_address = listener.local_addr().unwrap_or(bind_address);
    println!("Serving HelmAgent board at http://{local_address}");

    for stream in listener.incoming() {
        let stream = stream.context("accept board connection")?;
        handle_connection(stream, store)?;
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream, store: &TaskStore) -> Result<()> {
    let mut request = [0; 1024];
    let bytes_read = stream.read(&mut request).context("read board request")?;
    let request = std::str::from_utf8(&request[..bytes_read]).unwrap_or_default();
    if !is_allowed_board_request_host(request) {
        let response = forbidden_http_response();
        stream
            .write_all(response.as_bytes())
            .context("write forbidden board response")?;
        return Ok(());
    }

    let tasks = load_task_board_tasks(store)?;
    let body = render_task_board_html(&tasks);
    let response = board_http_response(&body);
    stream
        .write_all(response.as_bytes())
        .context("write board response")?;
    Ok(())
}

pub fn validate_loopback_bind_host(host: &str, port: u16) -> Result<()> {
    loopback_bind_address(host, port)?;
    Ok(())
}

pub fn loopback_bind_address(host: &str, port: u16) -> Result<SocketAddr> {
    let addresses = (host, port)
        .to_socket_addrs()
        .with_context(|| format!("resolve board host {host}:{port}"))?
        .collect::<Vec<_>>();

    if addresses.is_empty() {
        bail!("board host did not resolve: {host}");
    }
    if addresses.iter().any(|address| !address.ip().is_loopback()) {
        bail!("board serve only supports loopback hosts by default: {host}");
    }

    Ok(addresses[0])
}

pub fn is_allowed_board_request_host(request: &str) -> bool {
    request
        .lines()
        .find_map(host_header_value)
        .map(is_allowed_loopback_host_header)
        .unwrap_or(false)
}

pub fn board_http_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

pub fn forbidden_http_response() -> String {
    let body = "Forbidden\n";
    format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
}

fn host_header_value(line: &str) -> Option<&str> {
    line.split_once(':')
        .and_then(|(name, value)| name.eq_ignore_ascii_case("host").then_some(value.trim()))
}

fn is_allowed_loopback_host_header(value: &str) -> bool {
    let host = host_header_host(value);
    matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1")
}

fn host_header_host(value: &str) -> &str {
    if let Some(end) = value.strip_prefix('[').and_then(|rest| rest.find(']')) {
        return &value[..=end + 1];
    }

    value.split(':').next().unwrap_or(value)
}

fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }

    escaped
}
