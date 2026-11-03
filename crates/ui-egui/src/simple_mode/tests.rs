use egui::{Event, Key, Modifiers};
use serde_json::json;

use super::*;
use crate::canvas::ToolEvent;
use crate::canvas::tool_event;

fn services() -> crate::Services {
    crate::Services {
        import: Some(Box::new(|name: &str, bytes: &[u8], opts: &photocraft_io::ImportOptions| {
            photocraft_io::import_with_options(name, bytes, opts).map(|r| (r.document, r.warnings)).map_err(|e| e.to_string())
        })),
        ..Default::default()
    }
}

fn app() -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services());
    enable(&mut app);
    app
}

/// A 200x100 PNG with something on it, in a folder of its own that lives as long as the test.
fn picture(dir: &tempfile::TempDir) -> (String, Vec<u8>) {
    let path = dir.path().join("picture.png").to_string_lossy().into_owned();
    let mut s = photocraft_engine::Session::new();
    s.execute("file.new", json!({"width": 200, "height": 100, "background": "white"})).unwrap();
    s.execute("paint.stroke", json!({"points": [[0, 5], [199, 5]], "size": 4, "color": "#3366cc"})).unwrap();
    s.execute("file.export.quickExport", json!({"path": path})).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    (path, bytes)
}

fn opened(dir: &tempfile::TempDir) -> PhotocraftApp {
    let mut app = app();
    let (_, bytes) = picture(dir);
    app.open_bytes("picture.png", &bytes).unwrap();
    tick(&mut app);
    app
}

fn pen(app: &mut PhotocraftApp, pts: &[(f64, f64)]) {
    let (x, y) = pts[0];
    tool_event(app, ToolEvent::Down { x, y, pressure: 1.0 }, Modifiers::NONE);
    for &(x, y) in &pts[1..] {
        tool_event(app, ToolEvent::Move { x, y, pressure: 1.0 }, Modifiers::NONE);
    }
}

fn release(app: &mut PhotocraftApp, at: (f64, f64)) {
    tool_event(app, ToolEvent::Up { x: at.0, y: at.1 }, Modifiers::NONE);
}

/// A slightly wobbly straight stroke across the middle.
fn wobbly_line() -> Vec<(f64, f64)> {
    (0..=50).map(|i| (20.0 + f64::from(i) * 3.2, 50.0 + if i % 2 == 0 { 1.5 } else { -1.5 })).collect()
}

fn drawing_alpha(app: &PhotocraftApp, x: i32, y: i32) -> f32 {
    let st = app.session.active().unwrap();
    st.doc.layer(st.active_layer.unwrap()).unwrap().surface().unwrap().rgba(x, y)[3]
}

fn undo_steps(app: &PhotocraftApp) -> usize {
    app.session.active().unwrap().history.past_len()
}

#[test]
fn opening_adds_the_drawing_layer_above_the_image_without_an_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let app = opened(&dir);
    let st = app.session.active().unwrap();
    assert_eq!(st.doc.layers.len(), 2);
    assert_eq!(st.doc.layers.last().map(|l| l.id), st.active_layer, "on top, and active");
    assert!(!st.history.can_undo(), "nothing to undo yet");
    assert!(!st.is_dirty(), "nothing to lose yet");
}

#[test]
fn the_layer_stays_the_target_whatever_selects_another() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    let drawing = app.session.active().unwrap().active_layer.unwrap();
    let image = app.session.active().unwrap().doc.layers[0].id;
    app.run("layer.select", json!({"layer": image.0})).unwrap();
    tick(&mut app);
    assert_eq!(app.session.active().unwrap().active_layer, Some(drawing));
}

#[test]
fn the_eraser_only_erases_the_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    let st = app.session.active().unwrap();
    let image = st.doc.layers[0].id;
    let before = st.doc.layer(image).unwrap().surface().unwrap().clone();
    let line = wobbly_line();
    pen(&mut app, &line);
    release(&mut app, *line.last().unwrap());
    assert!(drawing_alpha(&app, 100, 50) > 0.5, "the pen drew");
    app.ui.tool = Tool::Eraser;
    app.run("tools.setBrush", json!({"brush": {"size": 40}})).unwrap();
    crate::paint_mouse::sync_tool_brush(&mut app);
    app.run("tools.setBrush", json!({"brush": {"size": 40}})).unwrap();
    let across = [(0.0, 50.0), (60.0, 50.0), (120.0, 50.0), (199.0, 50.0), (199.0, 5.0), (0.0, 5.0)];
    pen(&mut app, &across);
    release(&mut app, *across.last().unwrap());
    assert_eq!(drawing_alpha(&app, 100, 50), 0.0, "the drawing is erased");
    let st = app.session.active().unwrap();
    let after = st.doc.layer(image).unwrap().surface().unwrap();
    for (x, y) in [(100, 50), (100, 5), (0, 5), (199, 50), (10, 90)] {
        assert_eq!(after.rgba(x, y), before.rgba(x, y), "image pixel {x},{y}");
    }
}

#[test]
fn the_export_has_the_original_size_and_keeps_the_pixels_without_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    let line = wobbly_line();
    pen(&mut app, &line);
    release(&mut app, *line.last().unwrap());
    let out = dir.path().join("out.png").to_string_lossy().into_owned();
    export_to(&mut app, &out);
    assert!(!app.session.active().unwrap().is_dirty(), "exported: nothing to lose");
    let (_, original) = picture(&dir);
    let decode = |name: &str, bytes: &[u8]| photocraft_io::import_with_options(name, bytes, &Default::default()).unwrap().document;
    let (a, b) = (decode("picture.png", &original), decode("out.png", &std::fs::read(&out).unwrap()));
    assert_eq!((b.size.width, b.size.height), (200, 100));
    let (a, b) = (photocraft_compose::flatten(&a), photocraft_compose::flatten(&b));
    for y in 0..100 {
        for x in 0..200 {
            // The stroke, with the few pixels of faint edge a brush tip reaches past its radius.
            let drawn = (10..=192).contains(&x) && (38..=62).contains(&y);
            if !drawn {
                assert_eq!(a.get(x, y), b.get(x, y), "pixel {x},{y} keeps the image's value");
            }
        }
    }
    assert_ne!(a.get(100, 50), b.get(100, 50), "the drawing is in the PNG");
}

#[test]
fn a_failed_export_says_so_in_portuguese_and_keeps_the_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    let line = wobbly_line();
    pen(&mut app, &line);
    release(&mut app, *line.last().unwrap());
    i18n::set_current(i18n::Lang::from_pref("pt-br"));
    // A folder where the file should go: nothing can write there.
    let taken = dir.path().join("taken.png");
    std::fs::create_dir(&taken).unwrap();
    export_to(&mut app, &taken.to_string_lossy());
    assert!(app.ui.status_error && app.ui.status.contains("Não foi possível"), "{}", app.ui.status);
    assert!(app.session.active().unwrap().is_dirty());
}

#[test]
fn a_damaged_file_says_so_in_portuguese_and_leaves_the_app_as_it_was() {
    let mut app = app();
    i18n::set_current(i18n::Lang::from_pref("pt-br"));
    if let Err(e) = app.open_bytes("broken.png", b"not a png") {
        app.open_failed("broken.png", &e);
    }
    tick(&mut app);
    assert!(app.session.active().is_none());
    assert!(app.ui.status_error && app.ui.status.contains("Não foi possível abrir broken.png"), "{}", app.ui.status);
}

#[test]
fn files_other_than_images_are_refused() {
    let mut app = app();
    for name in ["a.psd", "b.pcraft", "c.cr2", "d.abr", "e"] {
        assert!(!openable(name), "{name}");
        assert!(app.open_bytes(name, b"x").is_err(), "{name}");
    }
    for name in ["a.PNG", "b.jpg", "c.jpeg", "d.webp", "e.tiff"] {
        assert!(openable(name), "{name}");
    }
}

#[test]
fn a_stroke_released_without_holding_is_not_corrected() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    let line = wobbly_line();
    pen(&mut app, &line);
    tick_hold(&mut app, 1000.0);
    tick_hold(&mut app, 1200.0);
    release(&mut app, *line.last().unwrap());
    tick_hold(&mut app, 5000.0);
    assert_eq!(undo_steps(&app), 1, "one stroke, no correction");
    assert!(app.session.journal.iter().all(|(id, _)| id != "paint.correctShape"));
}

#[test]
fn a_held_stroke_becomes_the_shape_and_undo_gives_the_stroke_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    app.run("tools.setBrush", json!({"brush": {"size": 2, "hardness": 1.0, "smoothing": {"amount": 0.0}}})).unwrap();
    let line = wobbly_line();
    pen(&mut app, &line);
    tick_hold(&mut app, 1000.0);
    tick_hold(&mut app, 1600.0);
    assert!(app.drag.is_none(), "the gesture ended at the correction");
    assert_eq!(undo_steps(&app), 2, "the stroke, then the shape");
    // Row 48 is on the wobble's way only: gone after the correction.
    let wobble = |app: &PhotocraftApp| (20..180).filter(|&x| drawing_alpha(app, x, 48) > 0.1).count();
    assert_eq!(wobble(&app), 0, "a straight line");
    // The release that follows does nothing more.
    tool_event(&mut app, ToolEvent::Move { x: 150.0, y: 90.0, pressure: 1.0 }, Modifiers::NONE);
    release(&mut app, (150.0, 90.0));
    assert_eq!(undo_steps(&app), 2);
    assert!(app.session.undo());
    assert!(wobble(&app) > 0, "the hand-drawn stroke is back");
    assert!(app.session.undo());
    assert_eq!((20..180).filter(|&x| drawing_alpha(&app, x, 50) > 0.1).count(), 0, "the second undo removes it");
    assert!(!app.session.undo(), "the automatic layer is not an undo step");
}

#[test]
fn a_held_scribble_stays_as_drawn_and_goes_on() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    let scribble: Vec<(f64, f64)> = (0..120).map(|i| (100.0 + 70.0 * (f64::from(i) * 1.7).sin(), 50.0 + 40.0 * (f64::from(i) * 2.3).cos())).collect();
    pen(&mut app, &scribble);
    tick_hold(&mut app, 1000.0);
    tick_hold(&mut app, 1600.0);
    assert!(app.drag.is_some(), "still drawing");
    assert_eq!(undo_steps(&app), 0);
    release(&mut app, *scribble.last().unwrap());
    assert_eq!(undo_steps(&app), 1, "the scribble, once");
}

#[test]
fn the_eraser_has_no_shape_correction() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    app.ui.tool = Tool::Eraser;
    let line = wobbly_line();
    pen(&mut app, &line);
    tick_hold(&mut app, 1000.0);
    tick_hold(&mut app, 1600.0);
    assert!(app.drag.is_some(), "still erasing");
    release(&mut app, *line.last().unwrap());
    assert!(app.session.journal.iter().all(|(id, _)| id != "paint.correctShape"));
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

fn kept(app: &PhotocraftApp, events: Vec<Event>) -> Vec<Event> {
    let mut raw = egui::RawInput { events, ..Default::default() };
    filter_input(app, &egui::Context::default(), &mut raw);
    raw.events
}

#[test]
fn only_the_listed_shortcuts_get_through() {
    let app = app();
    let cmd = Modifiers::COMMAND;
    for (k, m) in [(Key::Z, cmd), (Key::Z, cmd | Modifiers::SHIFT), (Key::Equals, cmd), (Key::Minus, cmd), (Key::Num0, cmd), (Key::Num1, cmd), (Key::O, cmd), (Key::Q, cmd), (Key::Space, Modifiers::NONE)] {
        assert_eq!(kept(&app, vec![key(k, m)]).len(), 1, "{k:?} {m:?}");
    }
    let hidden = [
        (Key::M, Modifiers::NONE),
        (Key::V, Modifiers::NONE),
        (Key::T, Modifiers::NONE),
        (Key::B, Modifiers::NONE),
        (Key::Tab, Modifiers::NONE),
        (Key::F, Modifiers::NONE),
        (Key::F7, Modifiers::NONE),
        (Key::N, cmd),
        (Key::S, cmd),
        (Key::S, cmd | Modifiers::SHIFT),
        (Key::T, cmd),
        (Key::A, cmd),
        (Key::C, cmd),
        (Key::V, cmd),
        (Key::J, cmd),
        (Key::L, cmd),
        (Key::M, cmd),
        (Key::U, cmd),
        (Key::W, cmd),
        (Key::Comma, cmd),
        (Key::Z, cmd | Modifiers::ALT),
        (Key::N, cmd | Modifiers::SHIFT),
    ];
    for (k, m) in hidden {
        assert!(kept(&app, vec![key(k, m)]).is_empty(), "{k:?} {m:?} must not get through");
    }
    assert!(kept(&app, vec![Event::Paste("x".into()), Event::Copy, Event::Cut, Event::Text("v".into())]).is_empty());
}

/// The real dispatch behind the filter: a hidden tool's letter leaves the tool alone, and the
/// shortcuts that stay do their job.
#[test]
fn the_filtered_keys_do_what_the_app_expects() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    let line = wobbly_line();
    pen(&mut app, &line);
    release(&mut app, *line.last().unwrap());
    let ctx = egui::Context::default();
    let press = |app: &mut PhotocraftApp, k: Key, m: Modifiers| {
        let mut raw = egui::RawInput { events: vec![Event::ModifiersChanged(m), key(k, m)], ..Default::default() };
        filter_input(app, &ctx, &mut raw);
        ctx.begin_pass(raw);
        crate::shortcuts::handle(app, &ctx);
        ctx.end_pass().textures_delta.clear();
        tick(app);
    };
    for k in [Key::M, Key::V, Key::T, Key::L, Key::G, Key::C, Key::U] {
        press(&mut app, k, Modifiers::NONE);
        assert_eq!(app.ui.tool, Tool::Brush, "{k:?}");
    }
    press(&mut app, Key::J, Modifiers::COMMAND);
    assert_eq!(app.session.active().unwrap().doc.layers.len(), 2, "Cmd+J adds no layer");
    assert_eq!(undo_steps(&app), 1);
    press(&mut app, Key::Z, Modifiers::COMMAND);
    assert_eq!(undo_steps(&app), 0, "Cmd+Z undoes");
    press(&mut app, Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(undo_steps(&app), 1, "Shift+Cmd+Z redoes");
    let zoom = app.point_zoom();
    press(&mut app, Key::Equals, Modifiers::COMMAND);
    assert_ne!(app.point_zoom(), zoom, "Cmd+= zooms");
}

#[test]
fn a_new_image_replaces_a_clean_one_and_asks_about_a_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = opened(&dir);
    let (_, bytes) = picture(&dir);
    app.open_bytes("second.png", &bytes).unwrap();
    tick(&mut app);
    assert_eq!(app.session.documents().len(), 1, "nothing to lose: replaced");
    assert_eq!(app.session.active().unwrap().doc.name, "second.png");
    let line = wobbly_line();
    pen(&mut app, &line);
    release(&mut app, *line.last().unwrap());
    app.open_bytes("third.png", &bytes).unwrap();
    tick(&mut app);
    assert_eq!(app.session.documents().len(), 2, "the drawing waits for an answer");
    assert!(replacing(&app).is_some());
}

#[test]
fn no_offer_to_import_another_editor_shortcuts() {
    let mut services = services();
    services.photoshop_shortcuts = Some(Box::new(|| None));
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), services);
    enable(&mut app);
    assert!(app.services.photoshop_shortcuts.is_none());
}

#[test]
fn no_update_check_starts() {
    let mut app = app();
    assert!(app.session.updates.check.is_none());
    assert!(app.run("help.checkForUpdates", json!({})).is_err(), "no service to check with");
}

#[test]
fn the_menu_lists_only_the_mode_commands() {
    let dir = tempfile::tempdir().unwrap();
    let app = opened(&dir);
    let items = crate::menus::menu_items(&app);
    let layout = crate::native_menu::simple_layout(&items, i18n::Lang::from_pref("pt-br"));
    let ids: Vec<&str> = layout.items().into_iter().map(|i| i.id.as_str()).collect();
    assert!(ids.iter().all(|id| menu_command(id) || *id == crate::native_menu::HIDE || *id == crate::native_menu::MINIMIZE), "{ids:?}");
    for id in ["file.open", "file.export.quickExportAsPng", "edit.undo", "edit.redo", "view.zoomIn", "view.zoomOut", "file.exit"] {
        assert!(ids.contains(&id), "{id} in {ids:?}");
    }
    let titles: Vec<&str> = layout.menus.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(titles.first().copied(), Some(crate::app_name::app_name()), "{titles:?}");
    assert!(titles.len() <= 5, "{titles:?}");
}

#[test]
fn the_bundle_name_comes_from_the_info_plist() {
    let plist = "<dict><key>CFBundleName</key><string>a</string><key>CFBundleDisplayName</key>\n  <string>b &amp; c</string></dict>";
    assert_eq!(crate::app_name::bundle_name(plist).as_deref(), Some("b & c"));
    assert_eq!(crate::app_name::bundle_name("<key>CFBundleName</key><string>a</string>").as_deref(), Some("a"));
    assert_eq!(crate::app_name::bundle_name("<dict></dict>"), None);
    assert_eq!(crate::app_name::app_name(), "teste", "no bundle here");
}

#[test]
fn the_interface_opens_in_portuguese_and_every_visible_string_has_a_translation() {
    let _app = app();
    let lang = i18n::Lang::from_pref("pt-br");
    for s in [
        "Open Image…",
        "Pen",
        "Eraser",
        "Color",
        "Thickness",
        "Undo",
        "Redo",
        "Fit",
        "Move View",
        "Export PNG…",
        "Zoom In",
        "Zoom Out",
        "Or drop an image here.",
        "Discard Drawing",
        "Keep Drawing",
        "PNG exported.",
        "Drawing",
        "Quit Without Exporting",
    ] {
        assert_ne!(i18n::tr(lang, s), s, "{s}");
    }
}
