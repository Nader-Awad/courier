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
        for key in phone_or_email_keys(chat_identifier) {
            if let Some(name) = self.handles.get(&key) {
                return Some(name.as_str());
            }
        }
        None
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

/// Returns the number of contact rows inserted (one contact with multiple
/// phone numbers counts as multiple insertions).
fn load_phones(conn: &Connection, out: &mut HashMap<String, String>) -> usize {
    let sql = "SELECT r.ZFIRSTNAME, r.ZLASTNAME, r.ZNICKNAME, r.ZORGANIZATION, p.ZFULLNUMBER \
               FROM ZABCDPHONENUMBER p \
               JOIN ZABCDRECORD r ON p.ZOWNER = r.Z_PK \
               WHERE p.ZFULLNUMBER IS NOT NULL";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return 0;
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
    let Ok(iter) = rows else { return 0 };
    let mut n = 0;
    for (first, last, nick, org, phone) in iter.flatten() {
        if let Some(name) = build_name(first, last, nick, org) {
            let mut inserted_any = false;
            for key in phone_keys(&phone) {
                if !out.contains_key(&key) {
                    out.insert(key, name.clone());
                    inserted_any = true;
                }
            }
            if inserted_any {
                n += 1;
            }
        }
    }
    n
}

fn load_emails(conn: &Connection, out: &mut HashMap<String, String>) -> usize {
    let sql = "SELECT r.ZFIRSTNAME, r.ZLASTNAME, r.ZNICKNAME, r.ZORGANIZATION, e.ZADDRESS \
               FROM ZABCDEMAILADDRESS e \
               JOIN ZABCDRECORD r ON e.ZOWNER = r.Z_PK \
               WHERE e.ZADDRESS IS NOT NULL";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return 0;
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
    let Ok(iter) = rows else { return 0 };
    let mut n = 0;
    for (first, last, nick, org, email) in iter.flatten() {
        if let Some(name) = build_name(first, last, nick, org) {
            let key = normalize_email(&email);
            if !key.is_empty() && !out.contains_key(&key) {
                out.insert(key, name);
                n += 1;
            }
        }
    }
    n
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

fn normalize_email(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

/// Produce all candidate lookup keys for a phone number so that a chat.db E.164
/// identifier (`+447011931883`) and an AddressBook national-format number
/// (`07011 931883`) collide on at least one key. Returned in priority order
/// (most specific first) so first-match wins during lookup.
fn phone_keys(raw: &str) -> Vec<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Vec::new();
    }
    let mut keys = Vec::with_capacity(4);
    keys.push(digits.clone());
    if digits.len() == 11 && digits.starts_with('1') {
        keys.push(digits[1..].to_string());
    }
    if digits.starts_with('0') && digits.len() >= 10 {
        keys.push(digits[1..].to_string());
    }
    if digits.len() > 10 {
        keys.push(digits[digits.len() - 10..].to_string());
    }
    keys
}

fn phone_or_email_keys(raw: &str) -> Vec<String> {
    if raw.contains('@') {
        let e = normalize_email(raw);
        if e.is_empty() { Vec::new() } else { vec![e] }
    } else {
        phone_keys(raw)
    }
}

pub fn normalize_identifier_for_debug(raw: &str) -> String {
    phone_or_email_keys(raw)
        .first()
        .cloned()
        .unwrap_or_default()
}

type SourceDiagnosis = Vec<(PathBuf, Result<(usize, usize), String>)>;

pub fn diagnose_sources() -> (usize, SourceDiagnosis) {
    let mut handles: HashMap<String, String> = HashMap::new();
    let mut results: SourceDiagnosis = Vec::new();
    for src in addressbook_sources() {
        match open_ro(&src) {
            Ok(conn) => {
                let phones = load_phones(&conn, &mut handles);
                let emails = load_emails(&conn, &mut handles);
                results.push((src, Ok((phones, emails))));
            }
            Err(e) => results.push((src, Err(e.to_string()))),
        }
    }
    (handles.len(), results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_e164_and_national_share_a_key() {
        let e164 = phone_keys("+15551234567");
        let freeform = phone_keys("(555) 123-4567");
        assert!(
            e164.iter().any(|k| freeform.contains(k)),
            "e164={e164:?} freeform={freeform:?}"
        );
    }

    #[test]
    fn uk_e164_matches_national_with_leading_zero() {
        let e164 = phone_keys("+447011931883");
        let national = phone_keys("07011 931883");
        assert!(
            e164.iter().any(|k| national.contains(k)),
            "e164={e164:?} national={national:?}"
        );
    }

    #[test]
    fn normalizes_email_case_and_whitespace() {
        assert_eq!(normalize_email("  Foo@Example.COM "), "foo@example.com");
    }

    #[test]
    fn phone_or_email_dispatches_correctly() {
        assert!(phone_or_email_keys("+15551234567").contains(&"5551234567".to_string()));
        assert_eq!(
            phone_or_email_keys("foo@example.com"),
            vec!["foo@example.com".to_string()]
        );
    }

    #[test]
    fn empty_input_produces_no_keys() {
        assert!(phone_keys("").is_empty());
        assert!(phone_or_email_keys("").is_empty());
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
