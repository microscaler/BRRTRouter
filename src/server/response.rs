use may_minihttp::Response;
use serde_json::Value;

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

pub fn write_handler_response(res: &mut Response, status: u16, body: Value, is_sse: bool) {
    let reason = status_reason(status);
    res.status_code(status as usize, reason);
    match body {
        Value::String(s) => {
            if is_sse {
                res.header("Content-Type: text/event-stream");
            } else {
                res.header("Content-Type: text/plain");
            }
            res.body_vec(s.into_bytes());
        }
        other => {
            res.header("Content-Type: application/json");
            res.body_vec(serde_json::to_vec(&other).unwrap());
        }
    }
}

pub fn write_json_error(res: &mut Response, status: u16, body: Value) {
    let reason = status_reason(status);
    res.status_code(status as usize, reason);
    res.header("Content-Type: application/json");
    res.body_vec(body.to_string().into_bytes());
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_reason() {
        assert_eq!(status_reason(200), "OK");
        assert_eq!(status_reason(404), "Not Found");
    }
}
