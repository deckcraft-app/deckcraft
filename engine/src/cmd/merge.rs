//! Merge Shapes (Shape Format ▸ Insert Shapes): union, combine, fragment, intersect and subtract
//! the outlines of the selected shapes. The result takes the formatting of the first selected
//! shape, like PowerPoint.

use deckcraft_geom::{PathEl, Rect, Xfrm};
use deckcraft_model::{CustomPath, Geom, Shape, ShapeId, ShapeKind};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use serde_json::{Value, json};

use super::*;
use crate::{Result, Session};

/// Closed polygons (slide points) describing a filled area (non-zero winding).
type Area = Vec<Vec<[f64; 2]>>;

pub const OPS: [(&str, &str); 5] =
    [("union", "Union"), ("combine", "Combine"), ("fragment", "Fragment"), ("intersect", "Intersect"), ("subtract", "Subtract")];

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "shape.merge",
        "Merge Shapes",
        ["Shape Format", "Insert Shapes", "Merge Shapes"],
        None,
        "{op: union|combine|fragment|intersect|subtract, ids?: shapes in selection order (the first one's formatting is kept)} → {ids}",
        has_two,
        merge
    )]
}

/// The filled outline of `s` in slide points (curves flattened).
fn area_of(doc: &deckcraft_model::Presentation, sel: &crate::Selection, s: &Shape) -> Area {
    let x = xfrm_of(doc, sel, s);
    let geo = deckcraft_render::shape_geometry(s, x.w, x.h);
    let a = x.affine();
    let mut out: Area = vec![];
    for sp in geo.paths.iter().filter(|p| p.fill != deckcraft_geom::preset::FillMode::None) {
        let mut cur: Vec<[f64; 2]> = vec![];
        let flush = |cur: &mut Vec<[f64; 2]>, out: &mut Area| {
            if cur.len() > 2 {
                out.push(std::mem::take(cur));
            } else {
                cur.clear();
            }
        };
        deckcraft_geom::preset_flatten(&sp.path, &mut |el| match el {
            PathEl::MoveTo(p) => {
                flush(&mut cur, &mut out);
                let q = a * p;
                cur.push([q.x, q.y]);
            }
            PathEl::LineTo(p) => {
                let q = a * p;
                cur.push([q.x, q.y]);
            }
            PathEl::ClosePath => flush(&mut cur, &mut out),
            _ => {}
        });
        flush(&mut cur, &mut out);
    }
    out
}

fn overlay(a: &Area, b: &Area, rule: OverlayRule) -> Area {
    if a.is_empty() && matches!(rule, OverlayRule::Union | OverlayRule::Xor) {
        return b.clone();
    }
    if b.is_empty() {
        return if matches!(rule, OverlayRule::Intersect) { vec![] } else { a.clone() };
    }
    a.overlay(b, rule, FillRule::NonZero).into_iter().flatten().filter(|c| c.len() > 2).collect()
}

fn area_size(a: &Area) -> f64 {
    a.iter()
        .map(|c| {
            let n = c.len();
            (0..n).map(|i| c[i][0] * c[(i + 1) % n][1] - c[(i + 1) % n][0] * c[i][1]).sum::<f64>() / 2.0
        })
        .sum::<f64>()
        .abs()
}

/// The merged areas for `op` (one per resulting shape).
pub fn merge_areas(op: &str, areas: &[Area]) -> Vec<Area> {
    let Some((first, rest)) = areas.split_first() else { return vec![] };
    let fold = |rule| rest.iter().fold(first.clone(), |acc, b| overlay(&acc, b, rule));
    let out = match op {
        "union" => vec![fold(OverlayRule::Union)],
        "combine" => vec![fold(OverlayRule::Xor)],
        "intersect" => vec![fold(OverlayRule::Intersect)],
        "subtract" => {
            let others = rest.iter().fold(Area::new(), |acc, b| overlay(&acc, b, OverlayRule::Union));
            vec![overlay(first, &others, OverlayRule::Difference)]
        }
        _ => {
            // Fragment: every region of the arrangement becomes its own shape.
            let mut pieces: Vec<Area> = vec![first.clone()];
            let mut covered = first.clone();
            for b in rest {
                let mut next = vec![];
                for p in &pieces {
                    next.push(overlay(p, b, OverlayRule::Intersect));
                    next.push(overlay(p, b, OverlayRule::Difference));
                }
                next.push(overlay(b, &covered, OverlayRule::Difference));
                covered = overlay(&covered, b, OverlayRule::Union);
                pieces = next;
            }
            // Split pieces into connected outer contours with their holes is not needed for
            // rendering; keep each piece as one shape.
            pieces
        }
    };
    out.into_iter().filter(|a| area_size(a) > 0.01).collect()
}

/// A custom-geometry box for `area`: its bounds and path data relative to them.
fn area_geom(area: &Area) -> Option<(Xfrm, Geom)> {
    let mut b: Option<Rect> = None;
    for p in area.iter().flatten() {
        let r = Rect::new(p[0], p[1], p[0], p[1]);
        b = Some(b.map_or(r, |x| x.union(r)));
    }
    let b = b?;
    let (w, h) = (b.width().max(0.01), b.height().max(0.01));
    let f = |v: f64| {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let mut d = String::new();
    for c in area {
        for (i, p) in c.iter().enumerate() {
            d += &format!("{}{} {} ", if i == 0 { "M " } else { "L " }, f(p[0] - b.x0), f(p[1] - b.y0));
        }
        d += "Z ";
    }
    let geom = Geom::Custom { paths: vec![CustomPath { w, h, d: d.trim_end().to_string(), ..Default::default() }] };
    Some((Xfrm::new(b.x0, b.y0, w, h), geom))
}

fn merge(s: &mut Session, p: &Value) -> Result<Value> {
    let op = str_param(p, "op").unwrap_or("union").to_ascii_lowercase();
    if !OPS.iter().any(|o| o.0 == op) {
        return Err(bad("shape.merge", format!("unknown op `{op}` (union, combine, fragment, intersect, subtract)")));
    }
    let ids = targets(s, p)?;
    let st = s.doc()?;
    let shapes: Vec<Shape> = ids.iter().filter_map(|id| st.shape(*id).cloned()).collect();
    let mergeable = |x: &Shape| matches!(x.kind, ShapeKind::Shape | ShapeKind::Picture { .. }) && !x.is_line();
    if shapes.len() < 2 || !shapes.iter().all(mergeable) {
        return Err(bad("shape.merge", "select two or more shapes (not lines, groups, tables or charts)"));
    }
    let areas: Vec<Area> = shapes.iter().map(|x| area_of(&st.doc, &st.selection, x)).collect();
    let results = merge_areas(&op, &areas);
    let template = shapes[0].clone();
    let mut new_ids = vec![];
    s.edit(|doc, sel| {
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad("shape.merge", "no slide"))?;
        let at = list.iter().position(|x| x.id == template.id).unwrap_or(list.len());
        let mut made = vec![];
        for (k, area) in results.iter().enumerate() {
            let Some((x, geom)) = area_geom(area) else { continue };
            let mut sh = template.clone();
            sh.id = ShapeId(0);
            sh.ph = None;
            sh.xfrm = Some(x);
            sh.geom = geom;
            sh.name = format!("Freeform {}", k + 1);
            // Text stays with the first piece only.
            if k > 0 {
                sh.text = None;
            }
            made.push(sh);
        }
        for sh in &mut made {
            sh.id = ShapeId(doc.alloc_id());
            new_ids.push(sh.id);
        }
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad("shape.merge", "no slide"))?;
        let at = at.min(list.len());
        for (k, sh) in made.into_iter().enumerate() {
            list.insert(at + k, sh);
        }
        list.retain(|x| !ids.contains(&x.id));
        sel.shapes = new_ids.clone();
        let _ = list;
        Ok(())
    })?;
    Ok(json!({"ids": new_ids.iter().map(|i| i.0).collect::<Vec<_>>()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Area {
        vec![vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]]]
    }

    #[test]
    fn boolean_ops_have_the_expected_areas() {
        let a = square(0.0, 0.0, 10.0);
        let b = square(5.0, 5.0, 10.0);
        let areas = [a, b];
        let size = |op| merge_areas(op, &areas).iter().map(area_size).sum::<f64>();
        assert!((size("union") - 175.0).abs() < 1e-6);
        assert!((size("intersect") - 25.0).abs() < 1e-6);
        assert!((size("subtract") - 75.0).abs() < 1e-6);
        assert!((size("combine") - 150.0).abs() < 1e-6);
        let frags = merge_areas("fragment", &areas);
        assert_eq!(frags.len(), 3);
        assert!((frags.iter().map(area_size).sum::<f64>() - 175.0).abs() < 1e-6);
        // Disjoint intersect is empty.
        assert!(merge_areas("intersect", &[square(0.0, 0.0, 1.0), square(5.0, 5.0, 1.0)]).is_empty());
    }
}
