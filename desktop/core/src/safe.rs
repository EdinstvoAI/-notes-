//! Сейф паролей — порт `PasswordSafe.swift` (03.11.2026): пароли, коды 2FA и секреты в зашифрованных
//! файлах хранилища `.safe/safe.enc` и `.safe/codes.enc` (коды — своим ключом). Формат тот же, что на маке,
//! байт в байт: «NSAFE1» + соль 16 + nonce 12 + шифр AES-256-GCM + тег 16; ключ — PBKDF2-SHA256, 600 000
//! раундов; внутри JSON как у `JSONEncoder` Swift (UUID заглавными, дата — секунды от 2001 года).
//! Забыт ключ — сейф не открыть никому.

use ring::{aead, hmac, pbkdf2, rand::{SecureRandom, SystemRandom}};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::path::Path;

pub const FILE: &str = ".safe/safe.enc";
pub const CODES_FILE: &str = ".safe/codes.enc";
const MAGIC: &[u8] = b"NSAFE1";
const ITERATIONS: u32 = 600_000;
/// Секунды между 1970 и 2001 — эпоха `Date` в Swift.
const SWIFT_EPOCH: f64 = 978_307_200.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Login,
    Totp,
    Secret,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub username: String,
    /// Пароль, секрет TOTP в base32 (или otpauth://) либо сам секрет.
    pub secret: String,
    pub url: String,
    pub note: String,
    /// Секунды от 1 января 2001 — как `Date` в JSON Swift.
    pub modified: f64,
}

impl Item {
    pub fn new(kind: Kind, title: &str) -> Self {
        Item { id: uuid(), kind, title: title.into(), username: String::new(), secret: String::new(), url: String::new(), note: String::new(), modified: now() }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    WrongPassword,
    Damaged,
    Io(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Failure::WrongPassword => write!(f, "ключ не подошёл"),
            Failure::Damaged => write!(f, "файл сейфа повреждён"),
            Failure::Io(e) => write!(f, "{e}"),
        }
    }
}

pub fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0) - SWIFT_EPOCH
}

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    SystemRandom::new().fill(&mut b).expect("системный генератор случайных чисел");
    b
}

/// UUID v4 заглавными — как `UUID().uuidString`.
pub fn uuid() -> String {
    let mut b = random::<16>();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02X}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

fn key(password: &str, salt: &[u8]) -> aead::LessSafeKey {
    let mut k = [0u8; 32];
    pbkdf2::derive(pbkdf2::PBKDF2_HMAC_SHA256, NonZeroU32::new(ITERATIONS).unwrap(), salt, password.as_bytes(), &mut k);
    aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_256_GCM, &k).unwrap())
}

/// Соль и nonce новые на каждую запись.
pub fn seal(items: &[Item], password: &str) -> Vec<u8> {
    let salt = random::<16>();
    let nonce = random::<12>();
    let mut body = serde_json::to_vec(items).unwrap();
    key(password, &salt).seal_in_place_append_tag(aead::Nonce::assume_unique_for_key(nonce), aead::Aad::empty(), &mut body).unwrap();
    [MAGIC, &salt, &nonce, &body].concat()
}

pub fn open(data: &[u8], password: &str) -> Result<Vec<Item>, Failure> {
    if data.len() <= MAGIC.len() + 16 + 28 || !data.starts_with(MAGIC) {
        return Err(Failure::Damaged);
    }
    let salt = &data[6..22];
    let nonce = aead::Nonce::try_assume_unique_for_key(&data[22..34]).map_err(|_| Failure::Damaged)?;
    let mut body = data[34..].to_vec();
    // Неверный ключ и подмена файла неотличимы: тег GCM не сходится.
    let json = key(password, salt).open_in_place(nonce, aead::Aad::empty(), &mut body).map_err(|_| Failure::WrongPassword)?;
    serde_json::from_slice(json).map_err(|_| Failure::Damaged)
}

/// None — файла нет.
pub fn load(vault: &Path, file: &str, password: &str) -> Result<Option<Vec<Item>>, Failure> {
    match std::fs::read(vault.join(file)) {
        Ok(data) => open(&data, password).map(Some),
        Err(_) => Ok(None),
    }
}

pub fn save(items: &[Item], vault: &Path, file: &str, password: &str) -> Result<(), Failure> {
    let path = vault.join(file);
    let io = |e: std::io::Error| Failure::Io(e.to_string());
    std::fs::create_dir_all(path.parent().unwrap()).map_err(io)?;
    // Атомарно: сначала рядом, потом на место — оборванная запись не портит сейф.
    let tmp = path.with_extension("enc.tmp");
    std::fs::write(&tmp, seal(items, password)).map_err(io)?;
    std::fs::rename(&tmp, &path).map_err(io)
}

pub fn exists(vault: &Path, file: &str) -> bool {
    vault.join(file).exists()
}

/// Ключи сейфа: свой мастер-пароль — один для обоих файлов; созданный приложением — два случайных.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keys {
    pub safe: String,
    pub codes: String,
}

impl Keys {
    /// Для другого компьютера — оба ключа одной строкой через пробел.
    pub fn transfer(&self) -> String {
        if self.safe == self.codes { self.safe.clone() } else { format!("{} {}", self.safe, self.codes) }
    }

    /// Что мог ввести человек: мастер-пароль целиком или два ключа через пробел.
    pub fn from(text: &str) -> Vec<Keys> {
        let mut out = vec![Keys { safe: text.into(), codes: text.into() }];
        let parts: Vec<&str> = text.split(' ').filter(|p| !p.is_empty()).collect();
        if parts.len() == 2 {
            out.push(Keys { safe: parts[0].into(), codes: parts[1].into() });
        }
        out
    }

    pub fn random() -> Keys {
        Keys { safe: generate(32), codes: generate(32) }
    }
}

/// Открыть оба файла. Старый сейф с кодами в safe.enc — раскладывает при сохранении (`needs_split`).
pub fn open_vault(vault: &Path, keys: &Keys) -> Result<(Vec<Item>, bool), Failure> {
    let main = load(vault, FILE, &keys.safe)?.unwrap_or_default();
    let codes = load(vault, CODES_FILE, &keys.codes)?.unwrap_or_default();
    let split = !exists(vault, FILE) || !exists(vault, CODES_FILE) || main.iter().any(|i| i.kind == Kind::Totp);
    Ok(([main, codes].concat(), split))
}

pub fn save_vault(vault: &Path, keys: &Keys, items: &[Item]) -> Result<(), Failure> {
    let (codes, main): (Vec<Item>, Vec<Item>) = items.iter().cloned().partition(|i| i.kind == Kind::Totp);
    save(&main, vault, FILE, &keys.safe)?;
    save(&codes, vault, CODES_FILE, &keys.codes)
}

/// Импорт CSV «Паролей» Apple (Title,URL,Username,Password,Notes,OTPAuth) — и такой же CSV из Edge и Chrome
/// (name,url,username,password,note): пароль — записью «пароли», код 2FA — записью «коды».
pub fn import_csv(text: &str) -> Vec<Item> {
    let rows = csv(text);
    let Some(header) = rows.first() else { return vec![] };
    let header: Vec<String> = header.iter().map(|h| h.trim().to_lowercase()).collect();
    let col = |names: &[&str]| names.iter().find_map(|n| header.iter().position(|h| h == n));
    let (t, u, n, p, note, otp) = (col(&["title", "name"]), col(&["url"]), col(&["username"]), col(&["password"]), col(&["notes", "note"]), col(&["otpauth"]));
    let mut items = vec![];
    for row in rows.iter().skip(1) {
        let v = |i: Option<usize>| i.and_then(|i| row.get(i)).cloned().unwrap_or_default();
        let title = if v(t).is_empty() { v(u) } else { v(t) };
        if !v(p).is_empty() {
            items.push(Item { username: v(n), secret: v(p), url: v(u), note: v(note), ..Item::new(Kind::Login, &title) });
        }
        if !v(otp).is_empty() {
            items.push(Item { username: v(n), secret: v(otp), ..Item::new(Kind::Totp, &title) });
        }
    }
    items
}

/// CSV с кавычками, запятыми и переносами внутри полей.
pub fn csv(text: &str) -> Vec<Vec<String>> {
    let (mut rows, mut row, mut field, mut quoted) = (vec![], vec![], String::new(), false);
    let text = text.replace("\r\n", "\n");
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') { field.push('"'); chars.next(); } else { quoted = false; }
            } else {
                field.push(c);
            }
        } else if c == '"' {
            quoted = true;
        } else if c == ',' {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' {
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Слабый мастер-пароль не запрещаем — предупреждаем: файл лежит на GitHub, его можно перебирать.
pub fn is_weak(p: &str) -> bool {
    let kinds = [p.chars().any(char::is_lowercase), p.chars().any(char::is_uppercase), p.chars().any(|c| c.is_ascii_digit()),
                 p.chars().any(|c| !c.is_alphanumeric())];
    p.chars().count() < 12 || kinds.iter().filter(|k| **k).count() < 3
}

/// Новый пароль без похожих знаков (0/O, 1/l), с цифрами и знаками.
pub fn generate(len: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789-_!@#%";
    let mut out = String::new();
    while out.len() < len {
        let b = random::<1>()[0];
        // Без перекоса: берём только байты из целых «оборотов» алфавита.
        if (b as usize) < 256 - 256 % CHARS.len() {
            out.push(CHARS[b as usize % CHARS.len()] as char);
        }
    }
    out
}

/// Коды 2FA по RFC 6238: HMAC-SHA1, 6 цифр, 30 секунд.
pub fn totp(secret: &str, unix: u64) -> Option<String> {
    let key = base32(secret).filter(|k| !k.is_empty())?;
    let mac = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, &key), &(unix / 30).to_be_bytes());
    let m = mac.as_ref();
    let o = (m[m.len() - 1] & 0x0f) as usize;
    let v = u32::from_be_bytes([m[o], m[o + 1], m[o + 2], m[o + 3]]) & 0x7fff_ffff;
    Some(format!("{:06}", v % 1_000_000))
}

/// Секрет из otpauth://…?secret=… или base32 с пробелами.
pub fn base32(text: &str) -> Option<Vec<u8>> {
    let raw = text.split_once("secret=").map(|(_, r)| r.split('&').next().unwrap_or("")).unwrap_or(text);
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let (mut bits, mut value, mut out) = (0u32, 0u32, vec![]);
    for c in raw.to_uppercase().chars().filter(|c| !matches!(c, ' ' | '=' | '-')) {
        let i = A.iter().position(|a| *a as char == c)? as u32;
        value = (value << 5) | i;
        bits += 5;
        if bits >= 8 {
            out.push(((value >> (bits - 8)) & 0xff) as u8);
            bits -= 8;
        }
        value &= (1 << bits) - 1;
    }
    Some(out)
}

/// Короткая метка хранилища (первые 8 байт SHA-256 пути) — имя записи ключей на этом компьютере.
pub fn vault_tag(path: &str) -> String {
    ring::digest::digest(&ring::digest::SHA256, path.as_bytes()).as_ref()[..8].iter().map(|b| format!("{b:02x}")).collect()
}
