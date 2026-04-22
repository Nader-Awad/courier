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
    pub rowid: i32,
    pub name: String,
    pub identifier: String,
    pub service: String,
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

    let mut summaries: Vec<ConversationSummary> = by_identifier
        .into_values()
        .map(|c| {
            let (name, resolved) = match c
                .display_name()
                .map(String::from)
                .or_else(|| contacts.lookup(&c.chat_identifier).map(String::from))
            {
                Some(name) => (name, true),
                None => (c.chat_identifier.clone(), false),
            };
            ConversationSummary {
                rowid: c.rowid,
                name,
                identifier: c.chat_identifier.clone(),
                service: c.service_name.unwrap_or_else(|| "Unknown".to_string()),
                resolved,
            }
        })
        .collect();

    summaries.sort_by(|a, b| {
        b.resolved
            .cmp(&a.resolved)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(summaries)
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
