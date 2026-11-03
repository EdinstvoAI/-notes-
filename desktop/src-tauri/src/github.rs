//! Вход в GitHub по коду (OAuth Device Flow) и приватный репозиторий `notes` — как `GitHub.swift` на маке.
//! Без своего сервера и без client secret: приложение показывает код, человек вводит его на
//! github.com/login/device, приложение спрашивает GitHub, подтвердили ли вход.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// OAuth App «<notes>» автора, Device Flow включён. Тот же, что на маке (`GitHubApp.swift`): не секрет.
pub const CLIENT_ID: &str = "Ov23lidTP0V2Y3bAknph";
/// Доступ к репозиториям: создать приватный `notes` и отправлять туда историю.
pub const SCOPE: &str = "repo";
pub const REPO: &str = "notes";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Code {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: Option<String>,
    /// Если в приложении GitHub включено истечение токенов: ключ продления и срок жизни токена.
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    interval: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub full_name: String,
    pub clone_url: String,
    pub private: bool,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).user_agent("notes-desktop").build()
}

pub fn request_code() -> Result<Code, String> {
    agent()
        .post("https://github.com/login/device/code")
        .set("Accept", "application/json")
        .send_form(&[("client_id", CLIENT_ID), ("scope", SCOPE)])
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())
}

/// Ждёт подтверждения и возвращает токен. `cancelled` — ушли с экрана: опрос прекращается.
pub fn wait_for_token(code: &Code, cancelled: impl Fn() -> bool) -> Result<Grant, String> {
    let mut interval = code.interval.max(1);
    loop {
        std::thread::sleep(Duration::from_secs(interval));
        if cancelled() {
            return Err("cancelled".into());
        }
        let reply: TokenReply = agent()
            .post("https://github.com/login/oauth/access_token")
            .set("Accept", "application/json")
            .send_form(&[
                ("client_id", CLIENT_ID),
                ("device_code", &code.device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        if let Some(grant) = reply.grant() {
            return Ok(grant);
        }
        match reply.error.as_deref() {
            Some("authorization_pending") => continue,
            // GitHub просит опрашивать реже: +5 секунд, как в его документации.
            Some("slow_down") => interval = reply.interval.unwrap_or(interval + 5),
            Some("expired_token") => return Err("код устарел, начните вход заново".into()),
            Some("access_denied") => return Err("вход отменён на github".into()),
            other => return Err(format!("github: {}", other.unwrap_or("пустой ответ"))),
        }
    }
}

fn api(method: &str, url: &str, token: &str) -> ureq::Request {
    agent()
        .request(method, url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
}

pub fn login(token: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct User {
        login: String,
    }
    let user: User = api("GET", "https://api.github.com/user", token).call().map_err(|e| e.to_string())?.into_json().map_err(|e| e.to_string())?;
    Ok(user.login)
}

/// Создать приватный `notes`; уже есть (GitHub отвечает 422) — подключиться к нему, новый не создаём.
pub fn ensure_notes(token: &str) -> Result<Repository, String> {
    let created = api("POST", "https://api.github.com/user/repos", token).send_json(serde_json::json!({
        "name": REPO, "private": true, "auto_init": false, "description": "заметки <notes>",
    }));
    match created {
        Ok(r) => r.into_json().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(422, _)) => {
            let who = login(token)?;
            api("GET", &format!("https://api.github.com/repos/{who}/{REPO}"), token)
                .call()
                .map_err(|e| format!("не нашли {who}/{REPO}: {e}"))?
                .into_json()
                .map_err(|e| e.to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_with_refresh() {
        let reply: TokenReply = serde_json::from_str(r#"{"access_token":"ghu_1","refresh_token":"ghr_1","expires_in":28800}"#).unwrap();
        let g = reply.grant().unwrap();
        assert_eq!((g.token.as_str(), g.refresh.as_deref()), ("ghu_1", Some("ghr_1")));
        assert!(g.expires.unwrap() > unix() + 28000);
        let old: TokenReply = serde_json::from_str(r#"{"access_token":"gho_1"}"#).unwrap();
        assert!(old.grant().unwrap().expires.is_none(), "без ключа продления токен вечный");
    }

    #[test]
    fn parses_device_code() {
        let code: Code = serde_json::from_str(r#"{"device_code":"d","user_code":"ABCD-1234","verification_uri":"https://github.com/login/device","expires_in":899,"interval":5}"#).unwrap();
        assert_eq!(code.user_code, "ABCD-1234");
        let reply: TokenReply = serde_json::from_str(r#"{"error":"slow_down","interval":10}"#).unwrap();
        assert_eq!((reply.error.as_deref(), reply.interval, reply.access_token), (Some("slow_down"), Some(10), None));
    }
}

/// Токен и, если GitHub его продлевает, ключ продления со сроком (порт `GitHubSession`, мак 03.11.2026:
/// без продления GitHub выкидывал раз в сутки).
pub struct Grant {
    pub token: String,
    pub refresh: Option<String>,
    /// Секунды с 1970, когда токен истечёт.
    pub expires: Option<u64>,
}

fn unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl TokenReply {
    fn grant(&self) -> Option<Grant> {
        Some(Grant { token: self.access_token.clone()?, refresh: self.refresh_token.clone(), expires: self.expires_in.map(|s| unix() + s) })
    }
}

/// Токен — в Диспетчере учётных данных; ключ продления и срок — там же, отдельной записью.
pub fn save_grant(grant: &Grant) -> Result<(), String> {
    crate::platform::secrets::save_token(&grant.token)?;
    match (&grant.refresh, grant.expires) {
        (Some(r), Some(e)) => crate::platform::secrets::save_refresh(&format!("{e} {r}")),
        _ => { crate::platform::secrets::forget_refresh(); Ok(()) }
    }
}

/// Токен для работы: истекает через 5 минут или раньше — продлеваем заранее. Без ключа продления токен вечный.
pub fn session_token() -> Option<String> {
    let token = crate::platform::secrets::token()?;
    let Some((expires, refresh)) = crate::platform::secrets::refresh().and_then(|s| s.split_once(' ').map(|(e, r)| (e.parse::<u64>().ok(), r.to_string()))) else {
        return Some(token);
    };
    if expires.is_some_and(|e| e > unix() + 300) {
        return Some(token);
    }
    let reply: Option<TokenReply> = agent()
        .post("https://github.com/login/oauth/access_token")
        .set("Accept", "application/json")
        .send_form(&[("client_id", CLIENT_ID), ("grant_type", "refresh_token"), ("refresh_token", &refresh)])
        .ok()
        .and_then(|r| r.into_json().ok());
    match reply.as_ref().and_then(TokenReply::grant) {
        Some(grant) => {
            let _ = save_grant(&grant);
            Some(grant.token)
        }
        None => Some(token),
    }
}
