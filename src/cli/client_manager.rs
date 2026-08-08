use crossterm::event::KeyCode;
use rusqlite::Connection;

use crate::invoicing::clients::{add_client, list_clients, update_client, ClientUpdate};
use crate::models::Client;

// Field indices for ClientForm — keep in sync with field order.
const NAME_IDX: usize = 0;
const EMAIL_IDX: usize = 1;
const ADDRESS_IDX: usize = 2;
const NOTES_IDX: usize = 3;

pub enum ClientAction {
    Continue,
    Close,
}

enum Screen {
    List,
    Add(ClientForm),
    Edit(ClientForm),
}

enum FormMode {
    Add,
    Edit,
}

struct ClientForm {
    fields: Vec<FormField>,
    focused: usize,
}

struct FormField {
    label: &'static str,
    value: String,
}

impl ClientForm {
    fn new_add() -> Self {
        Self::with_values(["", "", "", ""].map(str::to_string))
    }

    fn with_values(values: [String; 4]) -> Self {
        let labels = ["Name", "Email", "Address", "Notes"];
        Self {
            fields: labels
                .into_iter()
                .zip(values)
                .map(|(label, value)| FormField { label, value })
                .collect(),
            focused: 0,
        }
    }

    fn new_edit(client: &Client) -> Self {
        Self::with_values([
            client.name.clone(),
            client.email.clone().unwrap_or_default(),
            client.billing_address.clone().unwrap_or_default(),
            client.notes.clone().unwrap_or_default(),
        ])
    }

    /// The trimmed field, or `None` when it is blank.
    fn optional(&self, idx: usize) -> Option<String> {
        let value = self.fields[idx].value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    }
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

        // The screen is matched before the key, so a printable character on a
        // form types into the field instead of firing the list's binding.
        match &self.screen {
            Screen::List => self.handle_list_key(code, conn),
            Screen::Add(_) => self.handle_form_key(code, conn, FormMode::Add),
            Screen::Edit(_) => self.handle_form_key(code, conn, FormMode::Edit),
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
            KeyCode::Char('a') => self.screen = Screen::Add(ClientForm::new_add()),
            KeyCode::Char('e') => {
                if let Some(client) = self.clients.get(self.selection) {
                    self.screen = Screen::Edit(ClientForm::new_edit(client));
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => return ClientAction::Close,
            _ => {}
        }
        ClientAction::Continue
    }

    fn handle_form_key(
        &mut self,
        code: KeyCode,
        conn: &Connection,
        mode: FormMode,
    ) -> ClientAction {
        let form = match &mut self.screen {
            Screen::Add(f) | Screen::Edit(f) => f,
            Screen::List => return ClientAction::Continue,
        };

        match code {
            KeyCode::Esc => self.screen = Screen::List,
            KeyCode::Tab | KeyCode::Down => {
                form.focused = (form.focused + 1) % form.fields.len();
            }
            KeyCode::BackTab | KeyCode::Up => {
                form.focused = if form.focused == 0 {
                    form.fields.len() - 1
                } else {
                    form.focused - 1
                };
            }
            KeyCode::Char(c) => form.fields[form.focused].value.push(c),
            KeyCode::Backspace => {
                form.fields[form.focused].value.pop();
            }
            KeyCode::Enter => self.save_form(conn, mode),
            _ => {}
        }
        ClientAction::Continue
    }

    fn save_form(&mut self, conn: &Connection, mode: FormMode) {
        let form = match &self.screen {
            Screen::Add(f) | Screen::Edit(f) => f,
            Screen::List => return,
        };
        let name = form.fields[NAME_IDX].value.trim().to_string();
        let email = form.optional(EMAIL_IDX);
        let address = form.optional(ADDRESS_IDX);
        let notes = form.optional(NOTES_IDX);

        let saved = match mode {
            FormMode::Add => {
                // add_client validates nothing, so the blank-name refusal is
                // this screen's. update_client makes the same refusal itself.
                if name.is_empty() {
                    self.set_status("Name is required".into());
                    return;
                }
                add_client(
                    conn,
                    &name,
                    email.as_deref(),
                    address.as_deref(),
                    notes.as_deref(),
                )
                .map(|_| format!("Added client: {name}"))
            }
            FormMode::Edit => {
                let Some(client) = self.clients.get(self.selection) else {
                    return;
                };
                // The form holds every current value, so every field travels:
                // a blank optional one means "clear it", never "leave it".
                let update = ClientUpdate {
                    name: Some(name.clone()),
                    email: Some(email),
                    billing_address: Some(address),
                    notes: Some(notes),
                };
                update_client(conn, client.id, &update).map(|()| format!("Updated client: {name}"))
            }
        };
        match saved {
            Ok(message) => {
                self.reload(conn);
                self.screen = Screen::List;
                self.set_status(message);
            }
            Err(e) => self.set_status(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};
    use crate::invoicing::clients::{add_client, get_client};
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

    fn type_str(mgr: &mut ClientManager, conn: &Connection, text: &str) {
        for ch in text.chars() {
            mgr.handle_key(KeyCode::Char(ch), conn);
        }
    }

    fn form_values(mgr: &ClientManager) -> Vec<String> {
        match &mgr.screen {
            Screen::Add(form) | Screen::Edit(form) => {
                form.fields.iter().map(|f| f.value.clone()).collect()
            }
            Screen::List => panic!("not on a form"),
        }
    }

    fn client_named(conn: &Connection, name: &str) -> Client {
        list_clients(conn)
            .unwrap()
            .into_iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no client named {name}"))
    }

    /// Name, Email, Address, Notes typed into a fresh Add form.
    fn fill_add_form(mgr: &mut ClientManager, conn: &Connection, values: [&str; 4]) {
        mgr.handle_key(KeyCode::Char('a'), conn);
        for (i, value) in values.iter().enumerate() {
            if i > 0 {
                mgr.handle_key(KeyCode::Tab, conn);
            }
            type_str(mgr, conn, value);
        }
    }

    #[test]
    fn a_opens_the_add_form_with_empty_fields() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('a'), &conn);

        assert!(matches!(mgr.screen, Screen::Add(_)));
        assert_eq!(form_values(&mgr), ["", "", "", ""]);
    }

    #[test]
    fn enter_with_a_blank_name_reports_it_and_stays_on_the_form() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        fill_add_form(&mut mgr, &conn, ["  ", "ap@acme.test", "", ""]);
        mgr.handle_key(KeyCode::Enter, &conn);

        assert_eq!(mgr.status_message.as_deref(), Some("Name is required"));
        assert!(matches!(mgr.screen, Screen::Add(_)));
        assert!(list_clients(&conn).unwrap().is_empty());
    }

    #[test]
    fn enter_saves_a_client_and_returns_to_the_list() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        fill_add_form(
            &mut mgr,
            &conn,
            ["Acme Co", "ap@acme.test", "1 Main St", "Net 30"],
        );
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Added client: Acme Co"),
            "the status line names the client"
        );
        let saved = client_named(&conn, "Acme Co");
        assert_eq!(saved.email.as_deref(), Some("ap@acme.test"));
        assert_eq!(saved.billing_address.as_deref(), Some("1 Main St"));
        assert_eq!(saved.notes.as_deref(), Some("Net 30"));
        assert_eq!(mgr.clients.len(), 1, "the list reloaded");
    }

    #[test]
    fn blank_optional_fields_are_stored_as_null() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        fill_add_form(&mut mgr, &conn, ["Acme Co", "", "  ", ""]);
        mgr.handle_key(KeyCode::Enter, &conn);

        let saved = client_named(&conn, "Acme Co");
        assert_eq!(saved.email, None);
        assert_eq!(saved.billing_address, None);
        assert_eq!(saved.notes, None);
    }

    #[test]
    fn fields_are_trimmed() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        fill_add_form(
            &mut mgr,
            &conn,
            ["  Acme Co  ", "  ap@acme.test ", " 1 Main St ", " Net 30 "],
        );
        mgr.handle_key(KeyCode::Enter, &conn);

        let saved = client_named(&conn, "Acme Co");
        assert_eq!(saved.email.as_deref(), Some("ap@acme.test"));
        assert_eq!(saved.billing_address.as_deref(), Some("1 Main St"));
        assert_eq!(saved.notes.as_deref(), Some("Net 30"));
    }

    #[test]
    fn esc_cancels_without_writing() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        fill_add_form(&mut mgr, &conn, ["Acme Co", "", "", ""]);
        mgr.handle_key(KeyCode::Esc, &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert!(list_clients(&conn).unwrap().is_empty());
    }

    #[test]
    fn tab_and_backtab_cycle_the_four_fields() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('a'), &conn);

        for expected in [1, 2, 3, 0] {
            mgr.handle_key(KeyCode::Tab, &conn);
            assert_eq!(focused(&mgr), expected);
        }
        for expected in [3, 2, 1, 0] {
            mgr.handle_key(KeyCode::BackTab, &conn);
            assert_eq!(focused(&mgr), expected);
        }
    }

    #[test]
    fn a_printable_key_types_into_the_field_rather_than_triggering_the_list_binding() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('a'), &conn);
        type_str(&mut mgr, &conn, "aeq");
        mgr.handle_key(KeyCode::Backspace, &conn);

        assert!(matches!(mgr.screen, Screen::Add(_)), "still on the form");
        assert_eq!(form_values(&mgr)[0], "ae");
    }

    /// One fully-populated client, selected.
    fn seed_cedar(conn: &Connection) -> i64 {
        add_client(
            conn,
            "Cedar Systems",
            Some("ops@cedar.test"),
            Some("88 Cedar Way"),
            Some("Net 30"),
        )
        .unwrap()
    }

    /// Replace the focused field's contents.
    fn retype(mgr: &mut ClientManager, conn: &Connection, idx: usize, value: &str) {
        while focused(mgr) != idx {
            mgr.handle_key(KeyCode::Tab, conn);
        }
        for _ in 0..80 {
            mgr.handle_key(KeyCode::Backspace, conn);
        }
        type_str(mgr, conn, value);
    }

    #[test]
    fn e_opens_the_edit_form_prefilled_from_the_selected_row() {
        let (_d, conn) = test_conn();
        add_client(&conn, "Acme Co", None, None, None).unwrap();
        seed_cedar(&conn);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Down, &conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);

        assert!(matches!(mgr.screen, Screen::Edit(_)));
        assert_eq!(
            form_values(&mgr),
            ["Cedar Systems", "ops@cedar.test", "88 Cedar Way", "Net 30"]
        );

        // An absent field renders as an empty string, never as "None".
        mgr.handle_key(KeyCode::Esc, &conn);
        mgr.handle_key(KeyCode::Up, &conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        assert_eq!(form_values(&mgr), ["Acme Co", "", "", ""]);
    }

    #[test]
    fn enter_updates_the_client_and_returns_to_the_list() {
        let (_d, conn) = test_conn();
        let id = seed_cedar(&conn);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        retype(&mut mgr, &conn, EMAIL_IDX, "billing@cedar.test");
        mgr.handle_key(KeyCode::Enter, &conn);

        assert!(matches!(mgr.screen, Screen::List));
        assert_eq!(
            mgr.status_message.as_deref(),
            Some("Updated client: Cedar Systems")
        );
        let saved = get_client(&conn, id).unwrap();
        assert_eq!(saved.email.as_deref(), Some("billing@cedar.test"));
        assert_eq!(saved.billing_address.as_deref(), Some("88 Cedar Way"));
    }

    #[test]
    fn clearing_an_optional_field_writes_null() {
        let (_d, conn) = test_conn();
        let id = seed_cedar(&conn);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        retype(&mut mgr, &conn, EMAIL_IDX, "");
        mgr.handle_key(KeyCode::Enter, &conn);

        // Some(None), not None: None would have left the old address in place.
        assert_eq!(get_client(&conn, id).unwrap().email, None);
    }

    #[test]
    fn an_unchanged_field_still_round_trips_its_current_value() {
        let (_d, conn) = test_conn();
        let id = seed_cedar(&conn);
        let before = get_client(&conn, id).unwrap();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        mgr.handle_key(KeyCode::Enter, &conn);

        let after = get_client(&conn, id).unwrap();
        assert_eq!(after.name, before.name);
        assert_eq!(after.email, before.email);
        assert_eq!(after.billing_address, before.billing_address);
        assert_eq!(after.notes, before.notes);
    }

    #[test]
    fn a_blank_name_is_refused_in_the_data_layer_s_own_words() {
        let (_d, conn) = test_conn();
        let id = seed_cedar(&conn);
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        retype(&mut mgr, &conn, NAME_IDX, "   ");
        mgr.handle_key(KeyCode::Enter, &conn);

        assert_eq!(mgr.status_message.as_deref(), Some("Name is required"));
        assert!(matches!(mgr.screen, Screen::Edit(_)));
        assert_eq!(get_client(&conn, id).unwrap().name, "Cedar Systems");
    }

    #[test]
    fn a_data_layer_error_is_shown_verbatim_and_keeps_the_form_open() {
        let (_d, conn) = test_conn();
        seed_cedar(&conn);
        conn.execute_batch(
            "CREATE TRIGGER no_edits BEFORE UPDATE ON clients
             BEGIN SELECT RAISE(ABORT, 'clients are frozen'); END;",
        )
        .unwrap();

        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        retype(&mut mgr, &conn, NOTES_IDX, "Net 60");
        mgr.handle_key(KeyCode::Enter, &conn);

        let message = mgr.status_message.clone().unwrap();
        assert!(message.contains("clients are frozen"), "got: {message}");
        assert!(matches!(mgr.screen, Screen::Edit(_)), "the form stays open");
    }

    #[test]
    fn e_on_an_empty_list_does_nothing() {
        let (_d, conn) = test_conn();
        let mut mgr = manager(&conn);
        mgr.handle_key(KeyCode::Char('e'), &conn);
        assert!(matches!(mgr.screen, Screen::List));
    }

    fn focused(mgr: &ClientManager) -> usize {
        match &mgr.screen {
            Screen::Add(form) | Screen::Edit(form) => form.focused,
            Screen::List => panic!("not on a form"),
        }
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
