//! Blocking client: one request, one response line.

use std::io::{BufRead, BufReader, Write};

use anyhow::{bail, Context, Result};
use interprocess::local_socket::{prelude::*, Stream};
use serde_json::Value;

use crate::paths::socket_name;
use crate::protocol::{Request, Response};

pub struct Client {
    reader: BufReader<Stream>,
    next_id: u64,
}

impl Client {
    pub fn connect() -> Result<Client> {
        let stream = Stream::connect(socket_name()?).context("cannot reach rc-keeper daemon")?;
        Ok(Client {
            reader: BufReader::new(stream),
            next_id: 1,
        })
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let req = Request {
            id,
            method: method.to_string(),
            params,
        };
        let mut line = serde_json::to_string(&req)?;
        line.push('\n');
        self.reader.get_mut().write_all(line.as_bytes())?;
        self.reader.get_mut().flush()?;

        let mut buf = String::new();
        if self.reader.read_line(&mut buf)? == 0 {
            bail!("daemon closed the connection");
        }
        let resp: Response =
            serde_json::from_str(buf.trim_end()).context("bad response from daemon")?;
        if resp.id() != id {
            bail!("response id {} does not match request id {id}", resp.id());
        }
        resp.into_result().map_err(anyhow::Error::msg)
    }

    /// Returns `{"version": "...", "pid": n}`.
    pub fn ping(&mut self) -> Result<Value> {
        self.call("ping", Value::Null)
    }
}
