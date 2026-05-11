use crate::{domain::TaskRecord, output, store::TaskStore};
use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

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

pub fn serve_task_board(store: &TaskStore, host: &str, port: u16) -> Result<()> {
    let listener = TcpListener::bind((host, port))
        .with_context(|| format!("bind HelmAgent board server on {host}:{port}"))?;
    println!("Serving HelmAgent board at http://{host}:{port}");

    for stream in listener.incoming() {
        let stream = stream.context("accept board connection")?;
        handle_connection(stream, store)?;
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream, store: &TaskStore) -> Result<()> {
    let mut request = [0; 1024];
    let _ = stream.read(&mut request);
    let tasks = store.list_tasks()?;
    let body = render_task_board_html(&tasks);
    let response = board_http_response(&body);
    stream
        .write_all(response.as_bytes())
        .context("write board response")?;
    Ok(())
}

pub fn board_http_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
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
