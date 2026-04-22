use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OpenFlags};

pub struct ContactResolver {
    handles: HashMap<String, String>,
}

impl ContactResolver {
    pub fn load() -> Self {
        let mut handles = HashMap::new();
        for src in addressbook_sources() {
            if let Ok(conn) = open_ro(&src) {
                load_phones(&conn, &mut handles);
                load_emails(&conn, &mut handles);
            }
        }
        Self { handles }
    }

    pub fn lookup(&self, chat_identifier: &str) -> Option<&str> {
        let key = normalize_identifier(chat_identifier);
        if key.is_empty() {
            return None;
        }
        self.handles.get(&key).map(String::as_str)
    }
}

fn addressbook_sources() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let base = PathBuf::from(home).join("Library/Application Support/AddressBook/Sources");
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path().join("AddressBook-v22.abcddb"))
        .filter(|p| p.is_file())
        .collect()
}

fn open_ro(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
}

fn load_phones(conn: &Connection, out: &mut HashMap<String, String>) {
    let sql = "SELECT r.ZFIRSTNAME, r.ZLASTNAME, r.ZNICKNAME, r.ZORGANIZATION, p.ZFULLNUMBER \
               FROM ZABCDPHONENUMBER p \
               JOIN ZABCDRECORD r ON p.ZOWNER = r.Z_PK \
               WHERE p.ZFULLNUMBER IS NOT NULL";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return;
    };
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
        ))
    });
    let Ok(iter) = rows else { return };
    for (first, last, nick, org, phone) in iter.flatten() {
        if let Some(name) = build_name(first, last, nick, org) {
            let key = normalize_phone(&phone);
            if !key.is_empty() {
                out.entry(key).or_insert(name);
            }
        }
    }
}

fn load_emails(conn: &Connection, out: &mut HashMap<String, String>) {
    let sql = "SELECT r.ZFIRSTNAME, r.ZLASTNAME, r.ZNICKNAME, r.ZORGANIZATION, e.ZADDRESS \
               FROM ZABCDEMAILADDRESS e \
               JOIN ZABCDRECORD r ON e.ZOWNER = r.Z_PK \
               WHERE e.ZADDRESS IS NOT NULL";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return;
    };
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
        ))
    });
    let Ok(iter) = rows else { return };
    for (first, last, nick, org, email) in iter.flatten() {
        if let Some(name) = build_name(first, last, nick, org) {
            let key = normalize_email(&email);
            if !key.is_empty() {
                out.entry(key).or_insert(name);
            }
        }
    }
}

fn build_name(
    first: Option<String>,
    last: Option<String>,
    nick: Option<String>,
    org: Option<String>,
) -> Option<String> {
    let f = first.filter(|s| !s.is_empty());
    let l = last.filter(|s| !s.is_empty());
    match (f, l) {
        (Some(f), Some(l)) => return Some(format!("{f} {l}")),
        (Some(f), None) => return Some(f),
        (None, Some(l)) => return Some(l),
        _ => {}
    }
    if let Some(n) = nick.filter(|s| !s.is_empty()) {
        return Some(n);
    }
    org.filter(|s| !s.is_empty())
}

fn normalize_identifier(raw: &str) -> String {
    if raw.contains('@') {
        normalize_email(raw)
    } else {
        normalize_phone(raw)
    }
}

fn normalize_phone(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 11 && digits.starts_with('1') {
        digits[1..].to_string()
    } else {
        digits
    }
}

fn normalize_email(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_us_e164() {
        assert_eq!(normalize_phone("+15551234567"), "5551234567");
    }

    #[test]
    fn normalizes_us_freeform() {
        assert_eq!(normalize_phone("(555) 123-4567"), "5551234567");
    }

    #[test]
    fn normalizes_international_keeps_digits() {
        assert_eq!(normalize_phone("+44 20 7946 0018"), "442079460018");
    }

    #[test]
    fn normalizes_email_case_and_whitespace() {
        assert_eq!(normalize_email("  Foo@Example.COM "), "foo@example.com");
    }

    #[test]
    fn dispatch_picks_phone_or_email() {
        assert_eq!(normalize_identifier("+15551234567"), "5551234567");
        assert_eq!(normalize_identifier("foo@example.com"), "foo@example.com");
    }

    #[test]
    fn build_name_prefers_first_last() {
        let n = build_name(
            Some("Ada".to_string()),
            Some("Lovelace".to_string()),
            Some("Nick".to_string()),
            Some("Analytical Engine Co".to_string()),
        );
        assert_eq!(n.as_deref(), Some("Ada Lovelace"));
    }

    #[test]
    fn build_name_falls_back_to_organization() {
        let n = build_name(None, None, None, Some("Acme Corp".to_string()));
        assert_eq!(n.as_deref(), Some("Acme Corp"));
    }

    #[test]
    fn build_name_treats_empty_strings_as_missing() {
        let n = build_name(
            Some("".to_string()),
            Some("".to_string()),
            None,
            Some("Fallback".to_string()),
        );
        assert_eq!(n.as_deref(), Some("Fallback"));
    }
}
