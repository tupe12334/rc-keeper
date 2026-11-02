//! JSON-lines wire types shared by client and server.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Untagged: `{"id":n,"result":...}` or `{"id":n,"error":"..."}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    Ok { id: u64, result: Value },
    Err { id: u64, error: String },
}

impl Response {
    pub fn id(&self) -> u64 {
        match self {
            Response::Ok { id, .. } | Response::Err { id, .. } => *id,
        }
    }

    pub fn into_result(self) -> Result<Value, String> {
        match self {
            Response::Ok { result, .. } => Ok(result),
            Response::Err { error, .. } => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_round_trips() {
        let req = Request {
            id: 7,
            method: "get_logs".into(),
            params: json!({"id": "a"}),
        };
        let line = serde_json::to_string(&req).unwrap();
        assert_eq!(serde_json::from_str::<Request>(&line).unwrap(), req);
    }

    #[test]
    fn request_without_params_defaults_to_null() {
        let req: Request = serde_json::from_str(r#"{"id":1,"method":"ping"}"#).unwrap();
        assert_eq!(req.params, Value::Null);
    }

    #[test]
    fn ok_response_round_trips_including_null_result() {
        let resp = Response::Ok {
            id: 2,
            result: Value::Null,
        };
        let line = serde_json::to_string(&resp).unwrap();
        assert_eq!(line, r#"{"id":2,"result":null}"#);
        assert_eq!(serde_json::from_str::<Response>(&line).unwrap(), resp);
    }

    #[test]
    fn error_response_round_trips() {
        let resp = Response::Err {
            id: 3,
            error: "no instance".into(),
        };
        let line = serde_json::to_string(&resp).unwrap();
        assert_eq!(line, r#"{"id":3,"error":"no instance"}"#);
        let back: Response = serde_json::from_str(&line).unwrap();
        assert_eq!(back, resp);
        assert_eq!(back.into_result(), Err("no instance".to_string()));
    }

    #[test]
    fn into_result_returns_value_for_ok() {
        let resp: Response =
            serde_json::from_str(r#"{"id":4,"result":{"version":"1.2.3"}}"#).unwrap();
        assert_eq!(resp.id(), 4);
        assert_eq!(resp.into_result(), Ok(json!({"version": "1.2.3"})));
    }
}
