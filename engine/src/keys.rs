//! Keyboard: text-editing keys while editing text, nudges and shortcuts otherwise. Shared by the
//! UI, the control channel and MCP so a key does the same thing everywhere.

use serde_json::{Value, json};

use crate::tools::Mods;
use crate::{Result, Session};

/// Does a shortcut like `Cmd+Shift+]` match `key` + `mods`?
pub fn shortcut_matches(shortcut: &str, key: &str, mods: Mods) -> bool {
    let parts: Vec<&str> = shortcut.split('+').collect();
    let (k, ms) = match parts.as_slice() {
        [.., "", ""] => ("+", &parts[..parts.len().saturating_sub(2)]),
        _ => match parts.split_last() {
            Some((k, ms)) => (*k, ms),
            None => return false,
        },
    };
    let has = |m: &str| ms.iter().any(|x| x.eq_ignore_ascii_case(m));
    k.eq_ignore_ascii_case(key) && (has("cmd") || has("ctrl")) == mods.cmd && has("shift") == mods.shift && (has("alt") || has("option")) == mods.alt
}

impl Session {
    /// Handle one key press. `key` is a name (`Enter`, `Escape`, `Backspace`, `Delete`, `Tab`,
    /// `ArrowLeft`…, `Home`, `End`, `PageUp`, `PageDown`, `F5`) or a character (`b`, `]`).
    /// Returns `{handled: bool, …}`.
    pub fn key(&mut self, key: &str, mods: Mods) -> Result<Value> {
        let editing = self.active().is_some_and(|d| d.selection.text.is_some());
        let has_shapes = self.active().is_some_and(|d| !d.selection.shapes.is_empty());
        let k = key.to_ascii_lowercase();
        let k = k.trim_start_matches("arrow");
        if self.tool.drawing_freeform() && matches!(k, "enter" | "return" | "escape") {
            return self.finish_freeform(false);
        }
        if editing && !mods.cmd {
            let r = match k {
                "left" => Some(self.execute("text.move", &json!({"to": if mods.alt { "wordLeft" } else { "left" }, "extend": mods.shift}))),
                "right" => Some(self.execute("text.move", &json!({"to": if mods.alt { "wordRight" } else { "right" }, "extend": mods.shift}))),
                "up" => Some(self.execute("text.move", &json!({"to": if mods.alt { "paraStart" } else { "up" }, "extend": mods.shift}))),
                "down" => Some(self.execute("text.move", &json!({"to": if mods.alt { "paraEnd" } else { "down" }, "extend": mods.shift}))),
                "home" => Some(self.execute("text.move", &json!({"to": "lineStart", "extend": mods.shift}))),
                "end" => Some(self.execute("text.move", &json!({"to": "lineEnd", "extend": mods.shift}))),
                "backspace" => Some(self.execute("text.delete", &json!({"dir": if mods.alt { "wordBackward" } else { "backward" }}))),
                "delete" => Some(self.execute("text.delete", &json!({"dir": if mods.alt { "wordForward" } else { "forward" }}))),
                "enter" | "return" => Some(self.execute("text.insert", &json!({"text": if mods.shift { "\u{b}" } else { "\n" }}))),
                "escape" | "esc" => Some(self.execute("text.exit", &json!({}))),
                "tab" => {
                    // Tab at the start of a paragraph changes the list level; elsewhere it types a tab.
                    let at_start = self.active().and_then(|d| d.selection.text.as_ref()).is_some_and(|t| t.ordered().0.1 == 0 || t.is_range());
                    let in_cell = self.active().and_then(|d| d.selection.text.as_ref()).is_some_and(|t| t.cell.is_some());
                    if in_cell {
                        Some(self.next_cell(mods.shift))
                    } else if at_start {
                        Some(self.execute(if mods.shift { "format.outdent" } else { "format.indent" }, &json!({})))
                    } else {
                        Some(self.execute("text.insert", &json!({"text": "\t"})))
                    }
                }
                _ => None,
            };
            if let Some(r) = r {
                r?;
                return Ok(json!({"handled": true}));
            }
        }
        if !editing && has_shapes && !mods.cmd {
            let step = if mods.alt { 1.0 } else { 6.0 };
            let d = match k {
                "left" => Some((-step, 0.0)),
                "right" => Some((step, 0.0)),
                "up" => Some((0.0, -step)),
                "down" => Some((0.0, step)),
                _ => None,
            };
            if let Some((dx, dy)) = d {
                self.execute("shape.move", &json!({"dx": dx, "dy": dy}))?;
                return Ok(json!({"handled": true}));
            }
            match k {
                "backspace" | "delete" => {
                    self.execute("edit.delete", &json!({}))?;
                    return Ok(json!({"handled": true}));
                }
                "enter" | "return" | "f2" => {
                    self.execute("text.edit", &json!({}))?;
                    return Ok(json!({"handled": true}));
                }
                "escape" | "esc" => {
                    self.execute("edit.deselect", &json!({}))?;
                    return Ok(json!({"handled": true}));
                }
                "tab" => {
                    self.cycle_selection(!mods.shift)?;
                    return Ok(json!({"handled": true}));
                }
                _ => {}
            }
        }
        if !editing && !has_shapes && !mods.cmd {
            let r = match k {
                "pagedown" | "down" | "right" => Some("slide.next"),
                "pageup" | "up" | "left" => Some("slide.previous"),
                "home" => Some("slide.first"),
                "end" => Some("slide.last"),
                "enter" | "return" => Some("slide.new"),
                "tab" => {
                    self.cycle_selection(!mods.shift)?;
                    return Ok(json!({"handled": true}));
                }
                _ => None,
            };
            if let Some(id) = r {
                self.execute(id, &json!({}))?;
                return Ok(json!({"handled": true}));
            }
        }
        // Shortcuts from the registry.
        let key_name = match k {
            "enter" | "return" => "Enter",
            "escape" | "esc" => "Escape",
            "delete" => "Delete",
            "backspace" => "Backspace",
            "pageup" => "PageUp",
            "pagedown" => "PageDown",
            "home" => "Home",
            "end" => "End",
            "tab" => "Tab",
            "space" | " " => "Space",
            _ => key,
        };
        let found = crate::command_specs().iter().find(|c| c.shortcut.is_some_and(|s| shortcut_matches(s, key_name, mods))).map(|c| c.id);
        if let Some(id) = found {
            let r = self.execute(id, &json!({}))?;
            return Ok(json!({"handled": true, "command": id, "result": r}));
        }
        Ok(json!({"handled": false}))
    }

    /// Type text: into the text being edited, or (a shape selected) start editing it.
    pub fn type_text(&mut self, text: &str) -> Result<Value> {
        let st = self.doc()?;
        if st.selection.text.is_none() {
            let Some(id) = st.selection.shapes.first().copied() else { return Ok(json!({"handled": false})) };
            if st.selection.shapes.len() != 1 {
                return Ok(json!({"handled": false}));
            }
            // Typing on a selected shape replaces its text (PowerPoint behaviour).
            self.execute("text.edit", &json!({"id": id}))?;
            self.execute("text.selectAll", &json!({}))?;
        }
        self.execute("text.insert", &json!({"text": text}))
    }

    fn cycle_selection(&mut self, forward: bool) -> Result<()> {
        let st = self.doc()?;
        let ids: Vec<_> = st.shapes().iter().filter(|s| !s.hidden).map(|s| s.id).collect();
        if ids.is_empty() {
            return Ok(());
        }
        let cur = st.selection.shapes.first().and_then(|c| ids.iter().position(|x| x == c));
        let n = ids.len();
        let next = match cur {
            Some(i) if forward => (i + 1) % n,
            Some(i) => (i + n - 1) % n,
            None if forward => 0,
            None => n - 1,
        };
        let id = ids.get(next).copied();
        self.select(|_, sel| {
            sel.shapes = id.into_iter().collect();
            sel.text = None;
        })
    }

    fn next_cell(&mut self, back: bool) -> Result<Value> {
        let st = self.doc()?;
        let Some(t) = st.selection.text.clone() else { return Ok(Value::Null) };
        let Some((r, c)) = t.cell else { return Ok(Value::Null) };
        let Some(deckcraft_model::ShapeKind::Table(tb)) = st.shape(t.shape).map(|s| s.kind.clone()) else { return Ok(Value::Null) };
        let (nr, nc) = (tb.n_rows(), tb.n_cols());
        let idx = r * nc + c;
        if !back && idx + 1 >= nr * nc {
            // Tab in the last cell adds a row.
            self.execute("table.insertRowBelow", &json!({"id": t.shape, "row": r}))?;
            return self.execute("text.edit", &json!({"id": t.shape, "cell": [r + 1, 0]}));
        }
        let ni = if back { idx.saturating_sub(1) } else { idx + 1 };
        self.execute("text.edit", &json!({"id": t.shape, "cell": [ni / nc.max(1), ni % nc.max(1)]}))?;
        self.execute("text.selectAll", &json!({}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_parsing() {
        let m = |shift, alt, cmd| Mods { shift, alt, cmd };
        assert!(shortcut_matches("Cmd+B", "b", m(false, false, true)));
        assert!(!shortcut_matches("Cmd+B", "b", m(true, false, true)));
        assert!(shortcut_matches("Cmd+Shift+]", "]", m(true, false, true)));
        assert!(shortcut_matches("Cmd+Shift+.", ".", m(true, false, true)));
        assert!(shortcut_matches("Delete", "Delete", m(false, false, false)));
    }

    #[test]
    fn keys_drive_editing() {
        let mut s = Session::with_new();
        let title = s.doc().unwrap().current_slide().unwrap().shapes[0].id.0;
        s.execute("edit.select", &json!({"ids": [title]})).unwrap();
        s.type_text("Hi").unwrap();
        s.key("Enter", Mods::default()).unwrap();
        s.type_text("there").unwrap();
        s.key("Backspace", Mods::default()).unwrap();
        s.key("Escape", Mods::default()).unwrap();
        let t = s.execute("text.get", &json!({"id": title})).unwrap();
        assert_eq!(t["text"], "Hi\nther");
        let r = s.key("b", Mods { cmd: true, ..Default::default() }).unwrap();
        assert_eq!(r["command"], "format.bold");
        s.key("Right", Mods::default()).unwrap();
        s.key("Delete", Mods::default()).unwrap();
        assert_eq!(s.doc().unwrap().current_slide().unwrap().shapes.len(), 1);
    }
}
