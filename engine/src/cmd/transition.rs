//! Transitions tab.

use std::sync::Arc;

use deckcraft_model::Transition;
use deckcraft_model::anim::TRANSITIONS;
use serde_json::{Value, json};

use super::*;
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "transition.set",
            "Transition",
            ["Transitions", "Transition to This Slide"],
            None,
            "{kind: none|morph|fade|push|wipe|split|reveal|cut|randomBar|shape|uncover|cover|flash|… (transition.list), option?, duration?: ms, index?}",
            has_slide,
            set
        ),
        cmd!("transition.options", "Effect Options", ["Transitions", "Transition to This Slide"], None, "{option, index?}", has_slide, options),
        cmd!(
            "transition.timing",
            "Timing",
            ["Transitions", "Timing"],
            None,
            "{duration?: ms, onClick?: bool, after?: ms | null, index?}",
            has_slide,
            timing
        ),
        cmd!("transition.applyAll", "Apply To All", ["Transitions", "Timing"], None, "{}", has_slide, apply_all),
        cmd!(query "transition.list", "Transition Gallery", [], None, "{} → [{id, label, category, duration, options}]", always, list),
    ]
}

fn slide_indices(s: &Session, p: &Value) -> Result<Vec<usize>> {
    let st = s.doc()?;
    if let Some(i) = usize_param(p, "index") {
        return Ok(vec![i]);
    }
    let v: Vec<usize> = st.selection.slides.iter().filter_map(|id| st.doc.slide_index(*id)).collect();
    Ok(if v.is_empty() { vec![st.selection.slide] } else { v })
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let kind = str_param(p, "kind").ok_or_else(|| bad("transition.set", "missing `kind`"))?;
    let info =
        TRANSITIONS.iter().find(|t| t.0.eq_ignore_ascii_case(kind)).ok_or_else(|| bad("transition.set", format!("unknown transition `{kind}`")))?;
    let option = str_param(p, "option").map(String::from).or_else(|| info.4.first().map(|o| o.to_string())).unwrap_or_default();
    let duration = p.get("duration").and_then(Value::as_u64).map(|v| v.min(59_000) as u32);
    let list = slide_indices(s, p)?;
    s.edit(|doc, _| {
        for i in list {
            if let Some(sl) = doc.slides.get_mut(i) {
                let sl = Arc::make_mut(sl);
                let mut t = sl.transition.clone().unwrap_or_default();
                t.kind = info.0.to_string();
                t.option = option.clone();
                t.duration_ms = duration.unwrap_or(info.3);
                t.raw = None;
                sl.transition = if info.0 == "none" && t.advance_after_ms.is_none() && t.advance_on_click { None } else { Some(t) };
            }
        }
        Ok(())
    })?;
    Ok(json!({"kind": info.0}))
}

fn options(s: &mut Session, p: &Value) -> Result<Value> {
    let option = str_param(p, "option").ok_or_else(|| bad("transition.options", "missing `option`"))?.to_string();
    let list = slide_indices(s, p)?;
    s.edit(|doc, _| {
        for i in list {
            if let Some(sl) = doc.slides.get_mut(i)
                && let Some(t) = Arc::make_mut(sl).transition.as_mut()
            {
                t.option = option.clone();
            }
        }
        Ok(())
    })?;
    ok()
}

fn timing(s: &mut Session, p: &Value) -> Result<Value> {
    let list = slide_indices(s, p)?;
    s.edit(|doc, _| {
        for i in list {
            if let Some(sl) = doc.slides.get_mut(i) {
                let t = Arc::make_mut(sl).transition.get_or_insert_with(Transition::default);
                if let Some(d) = p.get("duration").and_then(Value::as_u64) {
                    t.duration_ms = d.min(59_000) as u32;
                }
                if let Some(c) = bool_param(p, "onClick") {
                    t.advance_on_click = c;
                }
                match p.get("after") {
                    Some(Value::Null) => t.advance_after_ms = None,
                    Some(v) => t.advance_after_ms = v.as_u64().map(|x| x.min(86_400_000) as u32),
                    None => {}
                }
            }
        }
        Ok(())
    })?;
    ok()
}

fn apply_all(s: &mut Session, _p: &Value) -> Result<Value> {
    s.edit(|doc, sel| {
        let t = doc.slides.get(sel.slide).and_then(|x| x.transition.clone());
        for sl in &mut doc.slides {
            Arc::make_mut(sl).transition.clone_from(&t);
        }
        Ok(())
    })?;
    ok()
}

fn list(_s: &mut Session, _p: &Value) -> Result<Value> {
    Ok(Value::Array(TRANSITIONS.iter().map(|t| json!({"id": t.0, "label": t.1, "category": t.2, "duration": t.3, "options": t.4})).collect()))
}
