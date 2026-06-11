//! Xtream Codes API client. Milestone 1 covers authentication only; the
//! catalog endpoints arrive in Milestone 2.

use crate::models::{ConnectionTestResult, XtreamAccountInfo};
use serde_json::Value;

/// `GET {server}/player_api.php?username={u}&password={p}` and interpret the
/// account-info response (spec §5.1).
pub async fn test_connection(
    server_url: &str,
    username: &str,
    password: &str,
) -> ConnectionTestResult {
    let client = match super::http_client() {
        Ok(c) => c,
        Err(e) => return ConnectionTestResult::failure(e),
    };
    let base = server_url.trim_end_matches('/');
    let url = format!("{base}/player_api.php");

    let response = client
        .get(&url)
        .query(&[("username", username), ("password", password)])
        .send()
        .await;

    let response = match response {
        Ok(r) => r,
        Err(_) => {
            return ConnectionTestResult::failure(format!(
                "Could not connect to {server_url}. Check the server address and your internet connection."
            ));
        }
    };

    if !response.status().is_success() {
        return ConnectionTestResult::failure(format!(
            "The server at {server_url} responded with HTTP {}. Check the server address.",
            response.status()
        ));
    }

    let body: Value = match response.json().await {
        Ok(v) => v,
        Err(_) => {
            return ConnectionTestResult::failure(format!(
                "The server at {server_url} did not return a valid Xtream Codes response. Check the server address."
            ));
        }
    };

    parse_auth_response(&body)
}

fn parse_auth_response(body: &Value) -> ConnectionTestResult {
    let user_info = &body["user_info"];
    if !value_truthy(&user_info["auth"]) {
        return ConnectionTestResult::failure(
            "Authentication failed. Check your username and password.",
        );
    }

    let status = value_to_string(&user_info["status"]);
    let info = XtreamAccountInfo {
        status: status.clone(),
        exp_date: value_to_i64(&user_info["exp_date"]),
        max_connections: value_to_i64(&user_info["max_connections"]),
        active_connections: value_to_i64(&user_info["active_cons"]),
    };

    let message = match status.as_deref() {
        Some(s) if s.eq_ignore_ascii_case("expired") => {
            "Connected, but the subscription has expired.".to_string()
        }
        _ => "Connected successfully.".to_string(),
    };

    ConnectionTestResult {
        success: true,
        message,
        account_info: Some(info),
    }
}

/// Xtream servers are loose with types: `auth` may be `1`, `"1"`, or `true`.
fn value_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        Value::String(s) => s == "1" || s.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

/// Numeric fields may arrive as numbers or numeric strings.
fn value_to_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
