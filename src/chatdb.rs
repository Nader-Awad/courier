use std::{collections::HashMap, path::Path};

use imessage_database::{
    error::table::TableError,
    tables::{
        chat::Chat,
        table::{Cacheable, get_connection},
    },
};

use crate::contacts::ContactResolver;

pub use imessage_database::util::dirs::default_db_path as default_db;

#[derive(Debug, Clone)]
pub struct ConversationSummary {
    pub name: String,
    pub identifiers: Vec<String>,
    pub rowids: Vec<i32>,
    pub services: Vec<String>,
    pub resolved: bool,
}

pub fn load_conversations(db_path: &Path) -> Result<Vec<ConversationSummary>, TableError> {
    let conn = get_connection(db_path)?;
    let chats: HashMap<i32, Chat> = Chat::cache(&conn)?;
    let contacts = ContactResolver::load();

    let mut by_identifier: HashMap<String, Chat> = HashMap::new();
    for (_rowid, chat) in chats {
        match by_identifier.get(&chat.chat_identifier) {
            Some(existing) if existing.rowid >= chat.rowid => {}
            _ => {
                by_identifier.insert(chat.chat_identifier.clone(), chat);
            }
        }
    }

    let mut by_record: HashMap<String, ConversationSummary> = HashMap::new();
    let mut singletons: Vec<ConversationSummary> = Vec::new();

    for c in by_identifier.into_values() {
        let display = c.display_name().map(String::from);
        let contact_ref = if display.is_none() {
            contacts.lookup(&c.chat_identifier).cloned()
        } else {
            None
        };
        let service = c.service_name.unwrap_or_else(|| "Unknown".to_string());
        let (name, resolved) = match (&display, &contact_ref) {
            (Some(n), _) => (n.clone(), true),
            (None, Some(cr)) => (cr.name.clone(), true),
            (None, None) => (c.chat_identifier.clone(), false),
        };
        let entry = ConversationSummary {
            name,
            identifiers: vec![c.chat_identifier.clone()],
            rowids: vec![c.rowid],
            services: vec![service],
            resolved,
        };

        match contact_ref {
            Some(cr) => {
                by_record
                    .entry(cr.record_key)
                    .and_modify(|existing| merge_into(existing, &entry))
                    .or_insert(entry);
            }
            None => singletons.push(entry),
        }
    }

    let mut summaries: Vec<ConversationSummary> =
        by_record.into_values().chain(singletons).collect();

    for s in &mut summaries {
        s.identifiers.sort();
        s.identifiers.dedup();
        s.rowids.sort();
        s.rowids.dedup();
    }

    summaries.sort_by(|a, b| {
        b.resolved
            .cmp(&a.resolved)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(summaries)
}

fn merge_into(dst: &mut ConversationSummary, src: &ConversationSummary) {
    dst.identifiers.extend(src.identifiers.iter().cloned());
    dst.rowids.extend(src.rowids.iter().cloned());
    for s in &src.services {
        if !dst.services.contains(s) {
            dst.services.push(s.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_db_path_under_home() {
        let p = default_db();
        assert!(p.ends_with("Library/Messages/chat.db"));
    }
}
