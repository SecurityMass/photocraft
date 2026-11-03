//! Shape correction: `paint.correctShape` replaces the stroke just painted with a clean outline.
//! The stroke stays one undo step behind the shape, so a first Undo gives the hand-drawn stroke
//! back and a second removes it. Recognizing the shape is `photocraft_paint::shape_fix`.

use photocraft_paint::render_stroke;
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

const CMD: &str = "paint.correctShape";

fn bad(msg: &str) -> EngineError {
    EngineError::BadParams { cmd: CMD.into(), msg: msg.into() }
}

fn correct_shape(s: &mut Session, p: &Value) -> Result<Value> {
    let pts = crate::brush_cmds::parse_points(p, CMD)?;
    if pts.len() < 2 {
        return Err(bad("needs at least two points"));
    }
    crate::brush_cmds::check_coords(&pts, CMD)?;
    let brush = crate::brush_cmds::resolve_brush(s, p, CMD)?;
    // The outline is already perfect: smoothing would only round its corners.
    let brush = photocraft_paint::BrushSettings { smoothing: Default::default(), ..brush };
    let zoom = p.get("zoom").and_then(Value::as_f64).map_or(1.0, |z| z as f32);
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let id = st.active_layer.ok_or_else(|| bad("no active layer"))?;
    // The layer as it was before the stroke: the latest history step is that stroke.
    let before = st.history.recorded_before().ok_or_else(|| bad("no stroke to correct"))?;
    let base = before.layer(id).and_then(|l| l.surface()).cloned().ok_or_else(|| bad("the stroke's layer has no pixels"))?;
    s.edit("Correct Shape", |doc, _| {
        let (surf, lock) = crate::channel_cmds::target_surface(doc, Some(id), &json!({}))?;
        *surf = base;
        render_stroke(surf, &brush, &pts, None, lock, zoom);
        Ok(())
    })?;
    Ok(Value::Null)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: CMD,
        label: "Correct Shape",
        menu: &[],
        shortcut: None,
        params: r##"{"points":[[x,y,pressure?],…],"zoom":num?} (the outline to draw, with the session brush, in place of the stroke just painted)"##,
        enabled: |s| s.active().map(|_| ()).ok_or_else(|| "no document open".into()),
        run: correct_shape,
        journal: true,
    }]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    fn alpha(s: &Session, x: i32, y: i32) -> f32 {
        let d = s.active().unwrap();
        d.doc.layer(d.active_layer.unwrap()).unwrap().surface().unwrap().rgba(x, y)[3]
    }

    /// A stroke along row 20, then a line along row 50 as its correction.
    fn stroke_and_correct(s: &mut Session) {
        s.execute("file.new", json!({"width": 200, "height": 100, "background": "transparent"})).unwrap();
        s.execute("tools.setBrush", json!({"brush": {"size": 6, "hardness": 1.0}})).unwrap();
        s.execute("paint.stroke", json!({"points": [[20, 20], [180, 20]]})).unwrap();
        assert!(alpha(s, 100, 20) > 0.9, "the hand-drawn stroke painted");
        assert_eq!(alpha(s, 100, 50), 0.0);
        let line: Vec<[f64; 2]> = (0..=160).map(|i| [20.0 + f64::from(i), 50.0]).collect();
        s.execute("paint.correctShape", json!({"points": line})).unwrap();
    }

    #[test]
    fn the_shape_replaces_the_stroke_with_the_same_thickness() {
        let mut s = Session::new();
        stroke_and_correct(&mut s);
        assert_eq!(alpha(&s, 100, 20), 0.0, "the hand-drawn stroke is gone");
        assert!(alpha(&s, 100, 50) > 0.9, "the shape is there");
        assert!(alpha(&s, 100, 48) > 0.9 && alpha(&s, 100, 45) == 0.0, "6 px wide, like the stroke");
    }

    #[test]
    fn undo_gives_the_stroke_back_and_a_second_undo_removes_it() {
        let mut s = Session::new();
        stroke_and_correct(&mut s);
        assert!(s.undo());
        assert!(alpha(&s, 100, 20) > 0.9, "the hand-drawn stroke is back");
        assert_eq!(alpha(&s, 100, 50), 0.0, "and the shape gone");
        assert!(s.undo());
        assert_eq!(alpha(&s, 100, 20), 0.0, "nothing left");
        assert!(s.redo() && s.redo());
        assert!(alpha(&s, 100, 50) > 0.9, "redo brings the shape back");
    }

    #[test]
    fn it_keeps_what_was_on_the_layer_before_the_stroke() {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 200, "height": 100, "background": "transparent"})).unwrap();
        s.execute("paint.stroke", json!({"points": [[10, 90], [190, 90]], "size": 6})).unwrap();
        s.execute("paint.stroke", json!({"points": [[20, 20], [180, 20]], "size": 6})).unwrap();
        s.execute("paint.correctShape", json!({"points": [[20, 50], [180, 50]], "size": 6})).unwrap();
        assert!(alpha(&s, 100, 90) > 0.9, "the earlier stroke stays");
        assert_eq!(alpha(&s, 100, 20), 0.0);
    }

    #[test]
    fn bad_params_are_errors_not_panics() {
        let mut s = Session::new();
        assert!(s.execute("paint.correctShape", json!({"points": [[1, 1], [2, 2]]})).is_err(), "no document");
        s.execute("file.new", json!({"width": 20, "height": 20})).unwrap();
        for p in [json!({}), json!({"points": []}), json!({"points": [[1, 1]]}), json!({"points": "x"}), json!({"points": [[1, 1], [2, 2]]})] {
            assert!(s.execute("paint.correctShape", p).is_err());
        }
    }
}
