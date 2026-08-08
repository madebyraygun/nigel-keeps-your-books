use crossterm::event::KeyCode;
use rusqlite::Connection;

use crate::invoicing::clients::list_clients;
use crate::models::Client;

pub enum ClientAction {
    Continue,
    Close,
}

enum Screen {
    List,
}

pub struct ClientManager {
    clients: Vec<Client>,
    selection: usize,
    scroll_offset: usize,
    last_visible_rows: usize,
    screen: Screen,
    status_message: Option<String>,
    /// Remaining keypresses before the status message is cleared.
    status_ttl: u8,
    greeting: String,
}

impl ClientManager {
    pub fn new(conn: &Connection, greeting: &str) -> Self {
        Self {
            clients: list_clients(conn).unwrap_or_default(),
            selection: 0,
            scroll_offset: 0,
            last_visible_rows: 20,
            screen: Screen::List,
            status_message: None,
            status_ttl: 0,
            greeting: greeting.to_string(),
        }
    }

    fn reload(&mut self, conn: &Connection) {
        self.clients = list_clients(conn).unwrap_or_default();
        if self.clients.is_empty() {
            self.selection = 0;
        } else {
            self.selection = self.selection.min(self.clients.len() - 1);
        }
    }

    fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_ttl = 3;
    }

    fn ensure_visible(&mut self, visible_rows: usize) {
        if self.selection < self.scroll_offset {
            self.scroll_offset = self.selection;
        } else if self.selection >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.selection - visible_rows + 1;
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, conn: &Connection) -> ClientAction {
        if self.status_ttl > 0 {
            self.status_ttl -= 1;
            if self.status_ttl == 0 {
                self.status_message = None;
            }
        }

        match &self.screen {
            Screen::List => self.handle_list_key(code, conn),
        }
    }

    fn handle_list_key(&mut self, code: KeyCode, _conn: &Connection) -> ClientAction {
        match code {
            KeyCode::Up => {
                self.selection = self.selection.saturating_sub(1);
                self.ensure_visible(self.last_visible_rows);
            }
            KeyCode::Down => {
                if !self.clients.is_empty() {
                    self.selection = (self.selection + 1).min(self.clients.len() - 1);
                    self.ensure_visible(self.last_visible_rows);
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => return ClientAction::Close,
            _ => {}
        }
        ClientAction::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::add_client;
    use crate::migrations::run_migrations;

    fn test_conn() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("t.db")).unwrap();
        init_db(&conn).unwrap();
        run_migrations(&conn).unwrap();
        (dir, conn)
    }

    fn manager(conn: &Connection) -> ClientManager {
        ClientManager::new(conn, "Hello, Dalton.")
    }

    fn is_close(action: ClientAction) -> bool {
        matches!(action, ClientAction::Close)
    }

    #[test]
    fn new_loads_clients_sorted_by_name() {
        let (_d, conn) = test_conn();
        for name in ["Cedar Systems", "Acme Co", "Blackwood & Sons"] {
            add_client(&conn, name, None, None, None).unwrap();
        }

        let mgr = manager(&conn);
        let names: Vec<&str> = mgr.clients.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Acme Co", "Blackwood & Sons", "Cedar Systems"]);
    }

    #[test]
    fn new_on_an_empty_book_has_no_selection_and_does_not_panic() {
        let (_d, conn) = test_conn();
        let mgr = manager(&conn);
        assert!(mgr.clients.is_empty());
        assert_eq!(mgr.selection, 0);
    }

    #[test]
    fn down_and_up_move_the_selection_and_clamp() {
        let (_d, conn) = test_conn();
        for name in ["Acme Co", "Blackwood & Sons"] {
            add_client(&conn, name, None, None, None).unwrap();
        }
        let mut mgr = manager(&conn);

        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, 1);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, 1, "Down past the end stays on the last row");
        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.selection, 0);
        mgr.handle_key(KeyCode::Up, &conn);
        assert_eq!(mgr.selection, 0, "Up from the top stays at the top");
    }

    #[test]
    fn esc_and_q_close() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        assert!(is_close(mgr.handle_key(KeyCode::Esc, &conn)));
        assert!(is_close(mgr.handle_key(KeyCode::Char('q'), &conn)));
    }

    #[test]
    fn keys_on_an_empty_list_do_not_panic() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        for code in [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Char('e'),
            KeyCode::Enter,
        ] {
            assert!(!is_close(mgr.handle_key(code, &conn)));
        }
        assert_eq!(mgr.selection, 0);
    }

    #[test]
    fn reload_clamps_the_selection_onto_the_shorter_list() {
        let (_d, conn) = test_conn();
        for name in ["Acme Co", "Blackwood & Sons"] {
            add_client(&conn, name, None, None, None).unwrap();
        }
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Down, &conn);
        assert_eq!(mgr.selection, 1);

        conn.execute("DELETE FROM clients WHERE name = 'Blackwood & Sons'", [])
            .unwrap();
        mgr.reload(&conn);
        assert_eq!(mgr.selection, 0);

        conn.execute("DELETE FROM clients", []).unwrap();
        mgr.reload(&conn);
        assert_eq!(mgr.selection, 0);
    }

    #[test]
    fn a_status_message_expires_after_three_keypresses() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.set_status("Added client: Acme Co".into());

        for _ in 0..2 {
            mgr.handle_key(KeyCode::Down, &conn);
            assert!(mgr.status_message.is_some());
        }
        mgr.handle_key(KeyCode::Down, &conn);
        assert!(mgr.status_message.is_none());
    }
}
