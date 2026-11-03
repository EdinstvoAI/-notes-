// По образцу NotesCore/Tests/NotesCoreTests/PasswordSafeTests.swift.
use notes_core::safe::*;

#[test]
fn round_trip() {
    let items = vec![Item { username: "makar".into(), secret: "тайна-123".into(), ..Item::new(Kind::Login, "почта") }];
    let data = seal(&items, "мастер");
    assert_eq!(open(&data, "мастер").unwrap(), items);
    assert_eq!(open(&data, "не тот"), Err(Failure::WrongPassword));
    let hay = String::from_utf8_lossy(&data);
    assert!(!hay.contains("тайна") && !hay.contains("makar"));
}

#[test]
fn damaged() {
    let mut data = seal(&[], "x");
    *data.last_mut().unwrap() ^= 1;
    assert_eq!(open(&data, "x"), Err(Failure::WrongPassword));
    assert_eq!(open("мусор".as_bytes(), "x"), Err(Failure::Damaged));
}

#[test]
fn totp_rfc6238() {
    let s = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
    assert_eq!(totp(s, 59).as_deref(), Some("287082"));
    assert_eq!(totp(s, 1_111_111_109).as_deref(), Some("081804"));
    assert_eq!(totp(&format!("otpauth://totp/x?secret={s}&issuer=y"), 59).as_deref(), Some("287082"));
    assert_eq!(totp("не base32", 59), None);
}

#[test]
fn import_apple_csv() {
    let csv = "Title,URL,Username,Password,Notes,OTPAuth\r\n\
        github.com,https://github.com,makark,\"p,a\"\"ss\",,otpauth://totp/x?secret=GEZDGNBVGY3TQOJQ\r\n\
        почта,https://mail.ru,me,123,\"строка 1\nстрока 2\",\r\n";
    let items = import_csv(csv);
    assert_eq!(items.iter().map(|i| i.kind).collect::<Vec<_>>(), [Kind::Login, Kind::Totp, Kind::Login]);
    assert_eq!(items[0].secret, "p,a\"ss");
    assert_eq!(items[2].note, "строка 1\nстрока 2");
}

/// Edge и Chrome выгружают name,url,username,password,note.
#[test]
fn import_edge_csv() {
    let items = import_csv("name,url,username,password,note\ngithub.com,https://github.com,me,pw,\n");
    assert_eq!((items.len(), items[0].title.as_str(), items[0].secret.as_str()), (1, "github.com", "pw"));
}

/// JSON — как у JSONEncoder Swift: мак должен открыть файл, записанный на Windows, и наоборот.
#[test]
fn swift_json_shape() {
    let swift = r#"[{"url":"","modified":783876000.5,"id":"E621E1F8-C36C-495A-93FC-0C247A3E6E5F","note":"","kind":"totp","title":"github","username":"makark","secret":"GEZDGNBVGY3TQOJQ"}]"#;
    let items: Vec<Item> = serde_json::from_str(swift).unwrap();
    assert_eq!(items[0].kind, Kind::Totp);
    let back = serde_json::to_string(&items).unwrap();
    for k in ["\"id\":\"E621E1F8", "\"kind\":\"totp\"", "\"modified\":783876000.5"] {
        assert!(back.contains(k), "{k}");
    }
    let id = uuid();
    assert!(id.len() == 36 && id == id.to_uppercase() && &id[14..15] == "4");
}

#[test]
fn two_files_and_split() {
    let d = tempfile::tempdir().unwrap();
    let keys = Keys::random();
    let items = vec![Item::new(Kind::Login, "a"), Item { secret: "GEZDGNBV".into(), ..Item::new(Kind::Totp, "b") }];
    save_vault(d.path(), &keys, &items).unwrap();
    assert!(exists(d.path(), FILE) && exists(d.path(), CODES_FILE));
    // Коды своим ключом: ключом паролей их не открыть.
    assert!(load(d.path(), CODES_FILE, &keys.safe).is_err());
    let (back, split) = open_vault(d.path(), &keys).unwrap();
    assert_eq!((back.len(), split), (2, false));
    let typed = Keys::from(&keys.transfer());
    assert!(typed.iter().any(|k| open_vault(d.path(), k).is_ok()));
    assert!(is_weak("qwerty") && !is_weak("Correct-horse-9"));
    assert_eq!(generate(20).len(), 20);
}
