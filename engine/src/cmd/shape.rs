//! Shape Format: position and size, rotation, flips, geometry, fill, outline, effects, styles,
//! names and alt text.

use deckcraft_color::{ColorTransform, Rgba, SchemeSlot};
use deckcraft_model::style::{Dash, Glow, Gradient, GradientShape, GradientStop, LineEnd, PatternFill, PictureFill, Reflection, Shadow};
use deckcraft_model::{ColorRef, Effects, Fill, Geom, Line, ShapeStyle};
use serde_json::{Value, json};

use super::*;
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "shape.connect",
            "Connect Shapes",
            [],
            None,
            "{from: shape id, to: shape id, fromSite?, toSite?: site index (default: the closest pair), id?: existing line to glue (else a new connector), preset?: straightConnector1|bentConnector3|curvedConnector3} → {id}",
            has_slide,
            connect
        ),
        cmd!(
            "shape.freeform",
            "Freeform",
            ["Insert", "Shapes"],
            None,
            "{points: [[x, y], …] slide pt (≥ 2), closed?: bool (filled shape), smooth?: bool (curve through the points)} → {id}",
            has_slide,
            freeform
        ),
        cmd!(query "shape.sites", "Connection Sites", [], None, "{id} → [[x, y]] connection sites in slide points", has_slide, sites),
        cmd!("shape.move", "Move", [], None, "{dx?, dy?: pt (relative) | x?, y?: pt (absolute), ids?}", has_selection, move_by),
        cmd!("shape.resize", "Size", ["Shape Format", "Size"], None, "{w?, h?: pt, lockAspect?: bool, ids?}", has_selection, resize),
        cmd!("shape.setBounds", "Position and Size", [], None, "{x, y, w, h, ids?}", has_selection, set_bounds),
        cmd!("shape.rotate", "Rotation", ["Shape Format", "Arrange", "Rotate"], None, "{deg?: absolute, by?: relative, ids?}", has_selection, rotate),
        cmd!("shape.flip", "Flip", ["Shape Format", "Arrange", "Rotate"], None, "{axis: horizontal|vertical, ids?}", has_selection, flip),
        cmd!("shape.adjust", "Adjust Shape", [], None, "{index, value (file units), ids?}", has_selection, adjust),
        cmd!("shape.change", "Change Shape", ["Shape Format", "Insert Shapes", "Edit Shape"], None, "{preset, ids?}", has_selection, change),
        cmd!(
            "shape.fill",
            "Shape Fill",
            ["Shape Format", "Shape Styles"],
            None,
            "{color? | none?: true | gradient?: {stops: [[pos, color]], angle?, kind?: linear|radial} | picture?: base64 | pattern?: {preset, fg, bg} | transparency?: 0..1 | reset?: true, ids?}",
            has_selection,
            fill
        ),
        cmd!(
            "shape.line",
            "Shape Outline",
            ["Shape Format", "Shape Styles"],
            None,
            "{color?, none?, width?: pt, dash?: solid|dot|dash|lgDash|dashDot|sysDot|sysDash, cap?, join?, head?: none|triangle|stealth|diamond|oval|arrow, tail?, reset?, ids?}",
            has_selection,
            line
        ),
        cmd!(
            "shape.effects",
            "Shape Effects",
            ["Shape Format", "Shape Styles"],
            None,
            "{shadow?: none|outer|inner|perspective|{color, blur, dist, dir}, glow?: none|{color, radius}, softEdges?: pt, reflection?: none|tight|half|full, bevel?: none|circle|…, reset?, ids?}",
            has_selection,
            effects
        ),
        cmd!(
            "shape.quickStyle",
            "Quick Styles",
            ["Home", "Drawing"],
            None,
            "{index: 0..41 (theme style row×accent), ids?}",
            has_selection,
            quick_style
        ),
        cmd!("shape.rename", "Rename", ["Selection Pane"], None, "{name, id?}", has_selection, rename),
        cmd!("shape.altText", "Alt Text", ["Shape Format", "Accessibility"], None, "{text, decorative?: bool, ids?}", has_selection, alt_text),
        cmd!("shape.visible", "Show/Hide", ["Selection Pane"], None, "{visible?: bool, ids?}", has_selection, visible),
        cmd!("shape.lock", "Lock", [], None, "{locked?: bool, ids?}", has_selection, lock),
        cmd!("shape.setDefault", "Set as Default Shape", [], None, "{}", has_selection, set_default),
        cmd!("picture.crop", "Crop", ["Picture Format", "Size"], None, "{left?, top?, right?, bottom?: fraction 0..1, ids?}", has_selection, crop),
        cmd!(
            "picture.adjust",
            "Corrections",
            ["Picture Format", "Adjust"],
            None,
            "{brightness?: -1..1, contrast?: -1..1, saturation?: 0..4, grayscale?: bool, transparency?: 0..1, reset?: bool, ids?}",
            has_selection,
            picture_adjust
        ),
        cmd!("picture.reset", "Reset Picture", ["Picture Format", "Adjust"], None, "{ids?}", has_selection, picture_reset),
        cmd!(
            "picture.change",
            "Change Picture",
            ["Picture Format", "Adjust"],
            None,
            "{path? | data: base64, name?, ids?}",
            has_selection,
            picture_change
        ),
        cmd!(
            "media.options",
            "Playback",
            ["Playback"],
            None,
            "{autoplay?, loop?, rewind?, acrossSlides?, hide?, fullScreen?, volume?: 0..1, trimStart?: ms, trimEnd?: ms, fadeIn?: ms, fadeOut?: ms, ids?}",
            has_selection,
            media_options
        ),
    ]
}

fn move_by(s: &mut Session, p: &Value) -> Result<Value> {
    let (dx, dy) = (f64_param(p, "dx"), f64_param(p, "dy"));
    let (ax, ay) = (f64_param(p, "x"), f64_param(p, "y"));
    edit_shapes(s, p, "shape.move", |sh| {
        if let Some(x) = sh.xfrm.as_mut() {
            x.x = ax.unwrap_or(x.x) + dx.unwrap_or(0.0);
            x.y = ay.unwrap_or(x.y) + dy.unwrap_or(0.0);
        }
        Ok(())
    })
}

fn resize(s: &mut Session, p: &Value) -> Result<Value> {
    let (w, h) = (f64_param(p, "w").map(|v| v.max(0.0)), f64_param(p, "h").map(|v| v.max(0.0)));
    let lock = bool_or(p, "lockAspect", false);
    edit_shapes(s, p, "shape.resize", |sh| {
        if let Some(x) = sh.xfrm.as_mut() {
            let ar = if x.h > 0.0 { x.w / x.h } else { 1.0 };
            let (cx, cy) = (x.x + x.w / 2.0, x.y + x.h / 2.0);
            match (w, h, lock) {
                (Some(w), _, true) => {
                    x.w = w;
                    x.h = if ar > 0.0 { w / ar } else { x.h };
                }
                (None, Some(h), true) => {
                    x.h = h;
                    x.w = h * ar;
                }
                _ => {
                    x.w = w.unwrap_or(x.w);
                    x.h = h.unwrap_or(x.h);
                }
            }
            // Rotated shapes resize about their centre; others keep the top-left.
            if x.rot != 0.0 {
                x.x = cx - x.w / 2.0;
                x.y = cy - x.h / 2.0;
            }
        }
        Ok(())
    })
}

fn set_bounds(s: &mut Session, p: &Value) -> Result<Value> {
    edit_shapes(s, p, "shape.setBounds", |sh| {
        if let Some(x) = sh.xfrm.as_mut() {
            x.x = f64_param(p, "x").unwrap_or(x.x);
            x.y = f64_param(p, "y").unwrap_or(x.y);
            x.w = f64_param(p, "w").unwrap_or(x.w).max(0.0);
            x.h = f64_param(p, "h").unwrap_or(x.h).max(0.0);
        }
        Ok(())
    })
}

fn rotate(s: &mut Session, p: &Value) -> Result<Value> {
    let (abs, by) = (f64_param(p, "deg"), f64_param(p, "by"));
    edit_shapes(s, p, "shape.rotate", |sh| {
        if let Some(x) = sh.xfrm.as_mut() {
            let r = abs.unwrap_or(x.rot) + by.unwrap_or(0.0);
            x.rot = ((r % 360.0) + 360.0) % 360.0;
        }
        Ok(())
    })
}

fn flip(s: &mut Session, p: &Value) -> Result<Value> {
    let v = str_param(p, "axis").unwrap_or("horizontal").starts_with('v');
    edit_shapes(s, p, "shape.flip", |sh| {
        if let Some(x) = sh.xfrm.as_mut() {
            if v {
                x.flip_v = !x.flip_v;
            } else {
                x.flip_h = !x.flip_h;
            }
        }
        Ok(())
    })
}

fn adjust(s: &mut Session, p: &Value) -> Result<Value> {
    let i = usize_param(p, "index").unwrap_or(0).min(16);
    let v = f64_param(p, "value").ok_or_else(|| bad("shape.adjust", "missing `value`"))?;
    edit_shapes(s, p, "shape.adjust", |sh| {
        if let Geom::Preset { name, adj } = &mut sh.geom {
            let defaults = deckcraft_geom::preset::info(name).map(|x| x.defaults).unwrap_or(&[]);
            while adj.len() <= i {
                let k = adj.len();
                adj.push(defaults.get(k).copied().unwrap_or(0.0));
            }
            if let Some(slot) = adj.get_mut(i) {
                *slot = v;
            }
        }
        Ok(())
    })
}

fn change(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "preset").ok_or_else(|| bad("shape.change", "missing `preset`"))?;
    let info = deckcraft_geom::preset::info(name).ok_or_else(|| bad("shape.change", format!("unknown preset `{name}`")))?;
    edit_shapes(s, p, "shape.change", |sh| {
        sh.geom = Geom::preset(info.name);
        Ok(())
    })
}

/// `{stops: [[pos 0–1, color], …], kind?: linear|radial|rectangular|path, angle?: deg (linear),
/// focus?: [l, t, r, b] fractions (radial/rectangular/path: where the first stop sits),
/// rotateWithShape?}`.
pub(crate) fn gradient_param(v: &Value) -> Option<Gradient> {
    let stops: Vec<GradientStop> = v
        .get("stops")?
        .as_array()?
        .iter()
        .filter_map(|s| {
            let a = s.as_array()?;
            Some(GradientStop { pos: a.first()?.as_f64()?.clamp(0.0, 1.0), color: color_value(a.get(1)?)? })
        })
        .collect();
    if stops.len() < 2 {
        return None;
    }
    let focus = v
        .get("focus")
        .and_then(Value::as_array)
        .and_then(|a| {
            let f = |i: usize| a.get(i).and_then(Value::as_f64).filter(|x| x.is_finite()).map(|x| x.clamp(0.0, 1.0));
            Some([f(0)?, f(1)?, f(2)?, f(3)?])
        })
        .unwrap_or([0.5, 0.5, 0.5, 0.5]);
    let mut stops = stops;
    stops.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    let path = |p: &str| GradientShape::Path { path: p.into(), focus };
    let shape = match v.get("kind").and_then(Value::as_str) {
        Some("radial" | "circle") => path("circle"),
        Some("rectangular" | "rect") => path("rect"),
        Some("path" | "shape") => path("shape"),
        _ => GradientShape::Linear {
            angle: v.get("angle").and_then(Value::as_f64).filter(|a| a.is_finite()).unwrap_or(90.0).rem_euclid(360.0),
            scaled: false,
        },
    };
    Some(Gradient { stops, shape, rotate_with_shape: v.get("rotateWithShape").and_then(Value::as_bool).unwrap_or(true) })
}

fn fill(s: &mut Session, p: &Value) -> Result<Value> {
    let new_fill: Option<Option<Fill>> = if bool_or(p, "reset", false) {
        Some(None)
    } else if bool_or(p, "none", false) {
        Some(Some(Fill::None))
    } else if let Some(c) = color_param(p, "color") {
        Some(Some(Fill::solid(c)))
    } else if let Some(g) = p.get("gradient") {
        Some(Some(Fill::Gradient(
            gradient_param(g).ok_or_else(|| bad("shape.fill", "gradient needs at least two `stops`: [[0, \"#fff\"], [1, \"#000\"]]"))?,
        )))
    } else if let Some(pt) = p.get("pattern") {
        Some(Some(Fill::Pattern(PatternFill {
            preset: pt.get("preset").and_then(Value::as_str).unwrap_or("pct50").to_string(),
            fg: pt.get("fg").and_then(color_value).unwrap_or(ColorRef::scheme(SchemeSlot::Accent1)),
            bg: pt.get("bg").and_then(color_value).unwrap_or(ColorRef::scheme(SchemeSlot::Bg1)),
        })))
    } else if p.get("picture").is_some() {
        let (name, bytes) = super::insert::media_bytes(&with_param(p, "data", p.get("picture").cloned().unwrap_or_default()), "shape.fill")?;
        let ct = super::insert::content_type(&name, &bytes);
        let media = s.edit(|doc, _| Ok(doc.add_media(&name, ct, bytes)))?;
        Some(Some(Fill::Picture(PictureFill { media, ..Default::default() })))
    } else {
        None
    };
    let transparency = f64_param(p, "transparency");
    if new_fill.is_none() && transparency.is_none() {
        return Err(bad("shape.fill", "give `color`, `none`, `gradient`, `pattern`, `picture`, `transparency` or `reset`"));
    }
    // Transparency on an inherited (style) fill needs the effective colour.
    let st = s.doc()?;
    let ctx_fill: Vec<(deckcraft_model::ShapeId, Option<Fill>)> = {
        let ctx = super::text::ctx_for(&st.doc, &st.selection);
        targets(s, p)?
            .into_iter()
            .filter_map(|id| {
                st.shape(id).map(|sh| {
                    (
                        id,
                        ctx.as_ref().and_then(|c| {
                            let (f, ph) = deckcraft_model::resolve::fill(c, sh);
                            f.map(|f| match f {
                                Fill::Solid { color } => Fill::solid(ColorRef::rgb(c.color(&color, ph))),
                                other => other,
                            })
                        }),
                    )
                })
            })
            .collect()
    };
    edit_shapes(s, p, "shape.fill", |sh| {
        if let Some(f) = &new_fill {
            sh.fill.clone_from(f);
        }
        if let Some(t) = transparency {
            let base = sh.fill.clone().or_else(|| ctx_fill.iter().find(|(i, _)| *i == sh.id).and_then(|(_, f)| f.clone()));
            if let Some(Fill::Solid { mut color }) = base {
                color.mods.retain(|m| !matches!(m, ColorTransform::Alpha(_)));
                if t > 0.0 {
                    color.mods.push(ColorTransform::Alpha(((1.0 - t.clamp(0.0, 1.0)) * 100_000.0).round() as i32));
                }
                sh.fill = Some(Fill::Solid { color });
            }
        }
        Ok(())
    })
}

fn line_end(v: &str) -> Option<LineEnd> {
    if v == "none" { None } else { Some(LineEnd { kind: v.to_string(), w: "med".into(), len: "med".into() }) }
}

fn line(s: &mut Session, p: &Value) -> Result<Value> {
    let reset = bool_or(p, "reset", false);
    let none = bool_or(p, "none", false);
    let color = color_param(p, "color");
    let width = f64_param(p, "width").map(|w| w.clamp(0.0, 1584.0));
    let dash = str_param(p, "dash").map(|d| Dash::Preset { name: d.to_string() });
    let head = str_param(p, "head").map(line_end);
    let tail = str_param(p, "tail").map(line_end);
    let cap = str_param(p, "cap").map(|c| match c {
        "round" => deckcraft_model::style::LineCap::Round,
        "square" => deckcraft_model::style::LineCap::Square,
        _ => deckcraft_model::style::LineCap::Flat,
    });
    let join = str_param(p, "join").map(|c| match c {
        "bevel" => deckcraft_model::style::LineJoin::Bevel,
        "miter" => deckcraft_model::style::LineJoin::Miter,
        _ => deckcraft_model::style::LineJoin::Round,
    });
    edit_shapes(s, p, "shape.line", |sh| {
        if reset {
            sh.line = None;
            return Ok(());
        }
        let l = sh.line.get_or_insert_with(Line::default);
        if none {
            l.fill = Some(Fill::None);
        }
        if let Some(c) = &color {
            l.fill = Some(Fill::solid(c.clone()));
        }
        if width.is_some() {
            l.width = width;
            if l.fill.is_none() || matches!(l.fill, Some(Fill::None)) && !none {
                l.fill = Some(Fill::solid(ColorRef::scheme(SchemeSlot::Tx1)));
            }
        }
        if dash.is_some() {
            l.dash.clone_from(&dash);
        }
        if let Some(h) = &head {
            l.head.clone_from(h);
        }
        if let Some(t) = &tail {
            l.tail.clone_from(t);
        }
        if cap.is_some() {
            l.cap = cap;
        }
        if join.is_some() {
            l.join = join;
        }
        Ok(())
    })
}

fn shadow_preset(v: &Value) -> Option<Option<Shadow>> {
    let black = |a: i32| ColorRef::rgb(Rgba::BLACK).with(ColorTransform::Alpha(a));
    let base = |inner: bool| Shadow {
        color: black(40000),
        blur: 4.0,
        dist: 3.0,
        dir: 45.0,
        inner,
        sx: 1.0,
        sy: 1.0,
        kx: 0.0,
        ky: 0.0,
        align: "tl".into(),
        rotate_with_shape: false,
    };
    match v {
        Value::String(s) => Some(match s.as_str() {
            "none" => None,
            "inner" => Some(base(true)),
            "perspective" => Some(Shadow { sy: 0.23, kx: -1200000.0 / 60000.0, dir: 90.0, dist: 0.0, blur: 6.0, align: "bl".into(), ..base(false) }),
            "bottom" => Some(Shadow { dir: 90.0, ..base(false) }),
            "center" => Some(Shadow { dist: 0.0, blur: 8.0, ..base(false) }),
            _ => Some(base(false)),
        }),
        Value::Object(o) => Some(Some(Shadow {
            color: o.get("color").and_then(color_value).unwrap_or(black(40000)),
            blur: o.get("blur").and_then(Value::as_f64).unwrap_or(4.0).clamp(0.0, 200.0),
            dist: o.get("dist").and_then(Value::as_f64).unwrap_or(3.0).clamp(0.0, 200.0),
            dir: o.get("dir").and_then(Value::as_f64).unwrap_or(45.0),
            inner: o.get("inner").and_then(Value::as_bool).unwrap_or(false),
            ..base(false)
        })),
        _ => None,
    }
}

fn effects(s: &mut Session, p: &Value) -> Result<Value> {
    let reset = bool_or(p, "reset", false);
    let shadow = p.get("shadow").and_then(shadow_preset);
    let glow: Option<Option<Glow>> = p.get("glow").map(|g| match g {
        Value::Object(o) => Some(Glow {
            color: o.get("color").and_then(color_value).unwrap_or(ColorRef::scheme(SchemeSlot::Accent1).with(ColorTransform::Alpha(40000))),
            radius: o.get("radius").and_then(Value::as_f64).unwrap_or(8.0).clamp(0.0, 150.0),
        }),
        Value::Number(n) => Some(Glow {
            color: ColorRef::scheme(SchemeSlot::Accent1).with(ColorTransform::Alpha(40000)),
            radius: n.as_f64().unwrap_or(8.0).clamp(0.0, 150.0),
        }),
        _ => None,
    });
    let soft = p.get("softEdges").map(|v| v.as_f64().filter(|x| *x > 0.0).map(|x| x.min(100.0)));
    let refl = str_param(p, "reflection").map(|r| match r {
        "none" => None,
        "half" => Some(Reflection { blur: 0.5, start_alpha: 0.5, end_alpha: 0.0, end_pos: 0.55, dist: 4.0 }),
        "full" => Some(Reflection { blur: 0.5, start_alpha: 0.5, end_alpha: 0.0, end_pos: 0.9, dist: 4.0 }),
        _ => Some(Reflection { blur: 0.5, start_alpha: 0.5, end_alpha: 0.0, end_pos: 0.35, dist: 0.0 }),
    });
    let bevel = str_param(p, "bevel").map(|b| if b == "none" { None } else { Some(b.to_string()) });
    edit_shapes(s, p, "shape.effects", |sh| {
        if reset {
            sh.effects = None;
            return Ok(());
        }
        let e = sh.effects.get_or_insert_with(Effects::default);
        if let Some(sw) = &shadow {
            match sw {
                Some(x) if x.inner => {
                    e.inner_shadow = Some(x.clone());
                    e.outer_shadow = None;
                }
                Some(x) => {
                    e.outer_shadow = Some(x.clone());
                    e.inner_shadow = None;
                }
                None => {
                    e.outer_shadow = None;
                    e.inner_shadow = None;
                }
            }
        }
        if let Some(g) = &glow {
            e.glow.clone_from(g);
        }
        if let Some(sf) = soft {
            e.soft_edge = sf;
        }
        if let Some(r) = &refl {
            e.reflection.clone_from(r);
        }
        if let Some(b) = &bevel {
            e.bevel.clone_from(b);
        }
        Ok(())
    })
}

/// Theme quick styles: 7 rows (outline only, solid fill variants, intense…) × 6 accents.
pub fn quick_style_for(index: usize) -> (ShapeStyle, Option<Fill>, Option<Line>) {
    let slots = [
        SchemeSlot::Dk1,
        SchemeSlot::Accent1,
        SchemeSlot::Accent2,
        SchemeSlot::Accent3,
        SchemeSlot::Accent4,
        SchemeSlot::Accent5,
        SchemeSlot::Accent6,
    ];
    let slot = slots.get(index % 7).copied().unwrap_or(SchemeSlot::Accent1);
    let row = index / 7;
    let mut st = ShapeStyle::accent(slot);
    let c = ColorRef::scheme(slot);
    match row {
        0 => {
            // Colored outline, light fill.
            st.fill_ref = (1, ColorRef::scheme(SchemeSlot::Lt1));
            st.line_ref = (2, c);
            st.font_ref.1 = Some(ColorRef::scheme(SchemeSlot::Dk1));
            (st, None, None)
        }
        1 => (st, None, None),
        2 => {
            st.fill_ref = (1, c.clone().with(ColorTransform::LumMod(20000)).with(ColorTransform::LumOff(80000)));
            st.line_ref = (0, c.clone());
            st.font_ref.1 = Some(c.with(ColorTransform::LumMod(50000)));
            (st, None, Some(Line::none()))
        }
        3 => {
            st.fill_ref = (2, c);
            (st, None, Some(Line::none()))
        }
        4 => {
            st.fill_ref = (3, c.clone());
            st.effect_ref = (3, c);
            (st, None, Some(Line::none()))
        }
        _ => {
            st.fill_ref = (1, c.clone().with(ColorTransform::LumMod(75000)));
            st.line_ref = (1, ColorRef::scheme(SchemeSlot::Lt1));
            (st, None, None)
        }
    }
}

fn quick_style(s: &mut Session, p: &Value) -> Result<Value> {
    let i = usize_param(p, "index").unwrap_or(1).min(41);
    let (st, f, l) = quick_style_for(i);
    edit_shapes(s, p, "shape.quickStyle", |sh| {
        sh.style = Some(st.clone());
        sh.fill.clone_from(&f);
        sh.line.clone_from(&l);
        sh.effects = None;
        if let Some(t) = sh.text.as_mut() {
            deckcraft_model::edit::format_all(t, &|r| r.fill = None);
        }
        Ok(())
    })
}

fn rename(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("shape.rename", "missing `name`"))?.to_string();
    edit_shapes(s, p, "shape.rename", |sh| {
        sh.name = name.clone();
        Ok(())
    })
}

fn alt_text(s: &mut Session, p: &Value) -> Result<Value> {
    let text = str_param(p, "text").unwrap_or("").to_string();
    let deco = bool_param(p, "decorative");
    edit_shapes(s, p, "shape.altText", |sh| {
        sh.descr = text.clone();
        if let Some(d) = deco {
            sh.decorative = d;
        }
        Ok(())
    })
}

fn visible(s: &mut Session, p: &Value) -> Result<Value> {
    let v = bool_param(p, "visible");
    edit_shapes(s, p, "shape.visible", |sh| {
        sh.hidden = !v.unwrap_or(sh.hidden);
        Ok(())
    })
}

fn lock(s: &mut Session, p: &Value) -> Result<Value> {
    let v = bool_param(p, "locked");
    edit_shapes(s, p, "shape.lock", |sh| {
        sh.locked = v.unwrap_or(!sh.locked);
        Ok(())
    })
}

fn set_default(s: &mut Session, _p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let Some(sh) = st.selected_shapes().first().map(|x| (*x).clone()) else { return ok() };
    // New shapes drawn in this session take this look (Set as Default Shape).
    s.default_look = Some(super::format::Painted {
        run: sh.text.as_ref().and_then(|t| t.paragraphs.first()).and_then(|p| p.runs.first()).map(|r| r.props.clone()),
        para: None,
        fill: sh.fill.clone(),
        line: sh.line.clone(),
        effects: sh.effects.clone(),
        style: sh.style.clone(),
    });
    ok()
}

fn crop(s: &mut Session, p: &Value) -> Result<Value> {
    let c = [f64_param(p, "left"), f64_param(p, "top"), f64_param(p, "right"), f64_param(p, "bottom")];
    edit_shapes(s, p, "picture.crop", |sh| {
        if let deckcraft_model::ShapeKind::Picture { fill } = &mut sh.kind {
            for (i, v) in c.iter().enumerate() {
                if let (Some(v), Some(slot)) = (v, fill.crop.get_mut(i)) {
                    *slot = v.clamp(-10.0, 0.95);
                }
            }
        }
        Ok(())
    })
}

fn picture_adjust(s: &mut Session, p: &Value) -> Result<Value> {
    edit_shapes(s, p, "picture.adjust", |sh| {
        if let deckcraft_model::ShapeKind::Picture { fill } = &mut sh.kind {
            let a = &mut fill.adjust;
            if bool_or(p, "reset", false) {
                *a = Default::default();
            }
            if let Some(v) = f64_param(p, "brightness") {
                a.brightness = v.clamp(-1.0, 1.0);
            }
            if let Some(v) = f64_param(p, "contrast") {
                a.contrast = v.clamp(-1.0, 1.0);
            }
            if let Some(v) = f64_param(p, "saturation") {
                a.saturation = Some(v.clamp(0.0, 4.0));
            }
            if let Some(v) = bool_param(p, "grayscale") {
                a.grayscale = v;
            }
            if let Some(v) = f64_param(p, "transparency") {
                fill.alpha = Some((1.0 - v).clamp(0.0, 1.0));
            }
        }
        Ok(())
    })
}

fn picture_reset(s: &mut Session, p: &Value) -> Result<Value> {
    edit_shapes(s, p, "picture.reset", |sh| {
        if let deckcraft_model::ShapeKind::Picture { fill } = &mut sh.kind {
            fill.adjust = Default::default();
            fill.crop = [0.0; 4];
            fill.alpha = None;
        }
        sh.effects = None;
        sh.line = None;
        Ok(())
    })
}

fn picture_change(s: &mut Session, p: &Value) -> Result<Value> {
    let (name, bytes) = super::insert::media_bytes(p, "picture.change")?;
    let ct = super::insert::content_type(&name, &bytes);
    let media = s.edit(|doc, _| Ok(doc.add_media(&name, ct, bytes)))?;
    edit_shapes(s, p, "picture.change", |sh| {
        if let deckcraft_model::ShapeKind::Picture { fill } = &mut sh.kind {
            fill.media = media;
            fill.crop = [0.0; 4];
        }
        Ok(())
    })
}

fn media_options(s: &mut Session, p: &Value) -> Result<Value> {
    edit_shapes(s, p, "media.options", |sh| {
        if let deckcraft_model::ShapeKind::Media(m) = &mut sh.kind {
            if let Some(v) = bool_param(p, "autoplay") {
                m.autoplay = v;
            }
            if let Some(v) = bool_param(p, "loop") {
                m.loop_play = v;
            }
            if let Some(v) = bool_param(p, "rewind") {
                m.rewind = v;
            }
            if let Some(v) = bool_param(p, "acrossSlides") {
                m.play_across_slides = v;
            }
            if let Some(v) = bool_param(p, "hide") {
                m.hide_while_not_playing = v;
            }
            if let Some(v) = bool_param(p, "fullScreen") {
                m.full_screen = v;
            }
            if let Some(v) = f64_param(p, "volume") {
                m.volume = v.clamp(0.0, 1.0);
            }
            let ms = |k: &str| p.get(k).and_then(Value::as_u64).map(|v| v.min(u32::MAX as u64) as u32);
            if let Some(v) = ms("trimStart") {
                m.trim_start_ms = v;
            }
            if let Some(v) = ms("trimEnd") {
                m.trim_end_ms = v;
            }
            if let Some(v) = ms("fadeIn") {
                m.fade_in_ms = v;
            }
            if let Some(v) = ms("fadeOut") {
                m.fade_out_ms = v;
            }
        }
        Ok(())
    })?;
    Ok(json!({}))
}

fn shape_id(p: &Value, key: &str) -> Option<deckcraft_model::ShapeId> {
    p.get(key).and_then(Value::as_u64).map(|v| deckcraft_model::ShapeId(v as u32))
}

fn sites(s: &mut Session, p: &Value) -> Result<Value> {
    let id = shape_id(p, "id").ok_or_else(|| bad("shape.sites", "missing `id`"))?;
    let st = s.doc()?;
    let sh = st.shape(id).ok_or_else(|| bad("shape.sites", "no such shape"))?;
    Ok(json!(crate::connect::sites(&st.doc, &st.selection, sh).iter().map(|q| [q.x, q.y]).collect::<Vec<_>>()))
}

fn connect(s: &mut Session, p: &Value) -> Result<Value> {
    let (Some(from), Some(to)) = (shape_id(p, "from"), shape_id(p, "to")) else { return Err(bad("shape.connect", "missing `from` / `to`")) };
    let st = s.doc()?;
    let site_list = |id| st.shape(id).map(|sh| crate::connect::sites(&st.doc, &st.selection, sh)).unwrap_or_default();
    let (a, b) = (site_list(from), site_list(to));
    if a.is_empty() || b.is_empty() {
        return Err(bad("shape.connect", "both shapes need connection sites (lines and groups have none)"));
    }
    let pick = |v: &[deckcraft_geom::Point], key: &str| usize_param(p, key).filter(|i| *i < v.len());
    let (ia, ib) = match (pick(&a, "fromSite"), pick(&b, "toSite")) {
        (Some(i), Some(j)) => (i, j),
        (fi, fj) => {
            let mut best = (f64::MAX, 0, 0);
            for (i, pa) in a.iter().enumerate().filter(|(i, _)| fi.is_none_or(|f| f == *i)) {
                for (j, pb) in b.iter().enumerate().filter(|(j, _)| fj.is_none_or(|f| f == *j)) {
                    let d = (*pa - *pb).hypot();
                    if d < best.0 {
                        best = (d, i, j);
                    }
                }
            }
            (best.1, best.2)
        }
    };
    let (p0, p1) = (a[ia], b[ib]);
    let id = match shape_id(p, "id") {
        Some(id) => id,
        None => {
            let preset = str_param(p, "preset").unwrap_or("straightConnector1");
            let r = s.execute("shape.insert", &json!({"preset": preset, "rect": [p0.x, p0.y, 1.0, 1.0]}))?;
            deckcraft_model::ShapeId(r.get("id").and_then(Value::as_u64).ok_or_else(|| bad("shape.connect", "insert failed"))? as u32)
        }
    };
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad("shape.connect", "no slide"))?;
        let sh = deckcraft_model::find_shape_mut(list, id).ok_or_else(|| bad("shape.connect", "no such line"))?;
        if !sh.is_line() {
            return Err(bad("shape.connect", "`id` is not a line or connector"));
        }
        sh.xfrm = Some(crate::tools::line_xfrm(p0, p1, 0.0));
        sh.kind = deckcraft_model::ShapeKind::Connector { start: Some((from, ia as u32)), end: Some((to, ib as u32)) };
        Ok(())
    })?;
    Ok(json!({"id": id.0, "fromSite": ia, "toSite": ib}))
}

/// Path data through `pts` (shape-local): straight segments, or a Catmull-Rom curve as cubics.
pub fn freeform_path(pts: &[deckcraft_geom::Point], closed: bool, smooth: bool) -> String {
    let f = |v: f64| {
        let s = format!("{:.2}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let mut d = format!("M {} {}", f(pts[0].x), f(pts[0].y));
    let n = pts.len();
    if smooth && n > 2 {
        let at = |i: isize| -> deckcraft_geom::Point {
            if closed { pts[i.rem_euclid(n as isize) as usize] } else { pts[i.clamp(0, n as isize - 1) as usize] }
        };
        let segs = if closed { n } else { n - 1 };
        for i in 0..segs as isize {
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            let c1 = p1 + (p2 - p0) / 6.0;
            let c2 = p2 - (p3 - p1) / 6.0;
            d += &format!(" C {} {} {} {} {} {}", f(c1.x), f(c1.y), f(c2.x), f(c2.y), f(p2.x), f(p2.y));
        }
    } else {
        for q in &pts[1..] {
            d += &format!(" L {} {}", f(q.x), f(q.y));
        }
    }
    if closed {
        d += " Z";
    }
    d
}

fn freeform(s: &mut Session, p: &Value) -> Result<Value> {
    let pts: Vec<deckcraft_geom::Point> = p
        .get("points")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| Some(deckcraft_geom::Point::new(v.get(0)?.as_f64()?, v.get(1)?.as_f64()?)))
                .filter(|q| q.x.is_finite() && q.y.is_finite())
                .take(20_000)
                .collect()
        })
        .unwrap_or_default();
    if pts.len() < 2 {
        return Err(bad("shape.freeform", "need at least two points"));
    }
    let closed = bool_or(p, "closed", false) && pts.len() > 2;
    let smooth = bool_or(p, "smooth", false);
    let mut bb = deckcraft_geom::Rect::from_points(pts[0], pts[0]);
    for q in &pts {
        bb = bb.union_pt(*q);
    }
    let (w, h) = (bb.width().max(1.0), bb.height().max(1.0));
    let local: Vec<deckcraft_geom::Point> = pts.iter().map(|q| deckcraft_geom::Point::new(q.x - bb.x0, q.y - bb.y0)).collect();
    let d = freeform_path(&local, closed, smooth);
    // Closed shapes take the default shape style, open paths the line style.
    let r = s.execute("shape.insert", &json!({"preset": if closed { "rect" } else { "line" }, "rect": [bb.x0, bb.y0, w, h]}))?;
    let id = deckcraft_model::ShapeId(r.get("id").and_then(Value::as_u64).ok_or_else(|| bad("shape.freeform", "insert failed"))? as u32);
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad("shape.freeform", "no slide"))?;
        let sh = deckcraft_model::find_shape_mut(list, id).ok_or_else(|| bad("shape.freeform", "insert failed"))?;
        sh.geom = Geom::Custom {
            paths: vec![deckcraft_model::CustomPath {
                w,
                h,
                d: d.clone(),
                fill: if closed { deckcraft_geom::preset::FillMode::Norm } else { deckcraft_geom::preset::FillMode::None },
                stroke: true,
            }],
        };
        sh.xfrm = Some(deckcraft_geom::Xfrm::new(bb.x0, bb.y0, w, h));
        sh.kind = deckcraft_model::ShapeKind::Shape;
        sh.name = format!("Freeform {}", id.0);
        Ok(())
    })?;
    Ok(json!({"id": id.0}))
}
