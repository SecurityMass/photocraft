//! The drawing app: open one image, draw over it by hand, export a PNG. Everything else the editor
//! can do stays in the code but out of reach: this module draws the only bar, lets through only
//! its keys, keeps the drawing on an automatic layer above the image, and corrects a stroke held
//! still into the shape it was meant to be.

use egui::{Key, Modifiers};
use photocraft_doc::{DocId, LayerId};
use photocraft_engine::BrushSettings;
use photocraft_paint::shape_fix::{self, HoldTimer};
use serde_json::{Value, json};

use crate::state::Tool;
use crate::{PhotocraftApp, canvas, i18n, menus, notices};

/// File types the app opens. Anything else could open dialogs this app doesn't have.
const IMAGE_EXTENSIONS: [&str; 11] = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff", "heic", "heif", "avif"];

/// Commands the native menu may show and run.
const MENU_COMMANDS: [&str; 9] =
    ["file.open", "file.export.quickExportAsPng", "file.exit", "edit.undo", "edit.redo", "view.zoomIn", "view.zoomOut", "view.fitOnScreen", "view.actualPixels"];

/// How far the pointer may drift during a hold, in screen points.
const HOLD_TOLERANCE: f64 = 4.0;

/// This mode's state on the app.
#[derive(Default)]
pub struct SimpleState {
    /// The drawing layer of each document seen.
    drawing: std::collections::HashMap<DocId, LayerId>,
    hold: HoldTimer,
}

/// Turn the mode on, before any file opens.
pub fn enable(app: &mut PhotocraftApp) {
    app.simple_mode = true;
    // No tab or progress card to show an open in progress: files open at once.
    app.background_jobs = false;
    app.session.edit_prefs(|p| {
        p.interface.language = "pt-br".into();
        p.general.automatically_check_updates = false;
        // The PNG keeps the image's own pixel values (and its profile).
        p.export.convert_to_srgb = false;
    });
    // No questions about colour profiles: an image keeps the one it came with.
    app.session.color.settings.ask_on_mismatch = false;
    app.session.color.settings.ask_on_missing = false;
    app.ui.tool = Tool::Brush;
    let current = app.session.tools.brush.clone();
    // A pen: hard, solid, thicker with pressure. Only a size chosen in the last run carries over.
    let chosen = current.size.is_finite() && current.size >= 1.0 && current.size != BrushSettings::default().size;
    app.session.tools.brush = BrushSettings {
        size: if chosen { current.size } else { 8.0 },
        hardness: 1.0,
        pressure_size: true,
        color: current.color,
        background: current.background,
        ..BrushSettings::default()
    };
    app.session.tools.brush_changed();
}

/// May the app open a file called `name`?
pub fn openable(name: &str) -> bool {
    let ext = std::path::Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    ext.is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.as_str()))
}

/// May the native menu show and run command `id`?
pub fn menu_command(id: &str) -> bool {
    MENU_COMMANDS.contains(&id)
}

/// The one message for a file that didn't open. The engine's reasons are in English, for the log.
pub fn open_failed(app: &mut PhotocraftApp, name: &str, reason: &str) {
    log::warn!("couldn't open {name}: {reason}");
    let message = i18n::fmt(tl!("Couldn't open {name}. The file may be damaged, or not a PNG, JPEG, WebP, BMP, GIF, TIFF or HEIC image."), &[("name", name)]);
    notices::error(app, message);
}

/// May this key press reach the app, with no text field or question open?
fn allowed_key(key: Key, m: Modifiers) -> bool {
    if m.alt {
        return false;
    }
    match (m.command, m.shift) {
        (true, false) => matches!(key, Key::Z | Key::O | Key::Q | Key::Plus | Key::Equals | Key::Minus | Key::Num0 | Key::Num1),
        (true, true) => matches!(key, Key::Z | Key::Plus | Key::Equals),
        (false, false) => matches!(key, Key::Space | Key::Escape),
        (false, true) => false,
    }
}

/// Take out of this frame's input every key that would reach something hidden. A text field being
/// typed in, or a question being answered, keeps its keys.
pub fn filter_input(app: &PhotocraftApp, ctx: &egui::Context, raw: &mut egui::RawInput) {
    if !app.simple_mode || ctx.text_edit_focused() || app.discard.is_some() || !app.ui.dialogs.is_empty() || replacing(app).is_some() {
        return;
    }
    raw.events.retain_mut(|e| match e {
        egui::Event::Key { key, pressed, modifiers, .. } => !*pressed || allowed_key(*key, *modifiers),
        egui::Event::Text(_) | egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_) | egui::Event::Ime(_) => false,
        egui::Event::ModifiersChanged(m) => {
            // Alt and Shift change what the pen does (colour picking, straight lines).
            (m.alt, m.shift) = (false, false);
            true
        }
        _ => true,
    });
}

/// The older document to close, when a new one has arrived and that one has a drawing not yet
/// exported, so closing it needs an answer.
fn replacing(app: &PhotocraftApp) -> Option<usize> {
    let docs = app.session.documents();
    let active = app.session.active_index()?;
    (docs.len() > 1).then(|| (0..docs.len()).find(|&i| i != active && docs.get(i).is_some_and(|d| d.is_dirty()))).flatten()
}

/// One document at a time: a new image replaces the last one, without losing a drawing unasked.
fn keep_one_document(app: &mut PhotocraftApp) {
    while app.session.documents().len() > 1 && replacing(app).is_none() {
        let Some(active) = app.session.active_index() else { return };
        let Some(old) = (0..app.session.documents().len()).find(|&i| i != active) else { return };
        app.session.close(old);
    }
    app.sync_views();
}

/// A document's drawing layer: created above the image the first time the document is seen,
/// without an undo step or unsaved changes, and always the layer tools act on.
pub fn ensure_drawing_layer(app: &mut PhotocraftApp) {
    let Some(st) = app.session.active() else { return };
    let (id, active, mode) = (st.doc.id, st.active_layer, st.doc.mode);
    match app.simple.drawing.get(&id).copied() {
        Some(layer) => {
            if active != Some(layer) {
                let _ = app.run("layer.select", json!({"layer": layer.0}));
            }
        }
        None => {
            // Colours must be able to land on the image, in any file's colour mode.
            if mode != photocraft_doc::ColorMode::Rgb
                && let Err(e) = app.run("image.mode.rgb", json!({}))
            {
                log::warn!("couldn't convert to RGB: {e}");
            }
            if let Err(e) = app.run("layer.new.layer", json!({"name": tl!("Drawing")})) {
                log::warn!("couldn't add the drawing layer: {e}");
                return;
            }
            let Some(st) = app.session.active_mut() else { return };
            let Some(layer) = st.active_layer else { return };
            st.history.clear();
            st.saved_revision = st.revision;
            app.simple.drawing.insert(id, layer);
        }
    }
}

/// Once a frame, before the UI draws.
pub fn tick(app: &mut PhotocraftApp) {
    if !app.simple_mode {
        return;
    }
    keep_one_document(app);
    ensure_drawing_layer(app);
    // Only the pen and the eraser draw, and the hand moves the view.
    if !matches!(app.ui.tool, Tool::Brush | Tool::Eraser | Tool::Hand) {
        app.ui.tool = Tool::Brush;
    }
}

/// Follow a pen stroke in progress: held still for long enough, it is corrected if it is a shape.
/// A stroke that isn't one simply goes on.
pub fn tick_hold(app: &mut PhotocraftApp, now_ms: f64) {
    let Some(at) = app.drag.as_ref().filter(|d| d.tool == Tool::Brush && !d.erase && !d.clear).and_then(|d| d.points.last()).map(|p| [p[0], p[1]]) else {
        app.simple.hold = HoldTimer::default();
        return;
    };
    let tolerance = HOLD_TOLERANCE / f64::from(app.point_zoom().max(0.01));
    if app.simple.hold.update(at, now_ms, tolerance) {
        correct_stroke(app);
    }
}

/// Replace the pen stroke in progress with the shape it was meant to be, if it is one. The stroke
/// is committed first, as its own undo step, and the pointer can then only release.
fn correct_stroke(app: &mut PhotocraftApp) {
    let Some(d) = app.drag.as_ref() else { return };
    let line: Vec<[f64; 2]> = d.points.iter().map(|p| [p[0], p[1]]).collect();
    let Some(shape) = shape_fix::recognize(&line) else { return };
    // The same thickness as the stroke: its typical pen pressure, all along.
    let mut pressures: Vec<f64> = d.points.iter().map(|p| p[2]).collect();
    pressures.sort_by(f64::total_cmp);
    let pressure = pressures.get(pressures.len() / 2).copied().unwrap_or(1.0);
    let revision = app.session.active().map(|st| st.revision);
    let Some(d) = app.drag.take() else { return };
    canvas::finish_gesture(app, d);
    if app.session.active().map(|st| st.revision) == revision {
        return;
    }
    let points: Vec<[f64; 3]> = shape.outline().into_iter().map(|p| [p[0], p[1], pressure]).collect();
    let zoom = app.point_zoom();
    if let Err(e) = app.run("paint.correctShape", json!({"points": points, "zoom": zoom})) {
        log::warn!("couldn't correct the stroke: {e}");
    }
}

/// Export the image with its drawing, asking where.
pub fn export_png(app: &mut PhotocraftApp) -> Result<Value, String> {
    let st = app.session.active().ok_or("no document")?;
    let stem = st.doc.name.rsplit_once('.').map_or(st.doc.name.as_str(), |(a, _)| a);
    let (suggested, doc) = (format!("{stem}.png"), st.doc.id);
    app.pick_save(&suggested, move |app, path| {
        app.refocus(doc)?;
        export_to(app, std::path::Path::new(&path).with_extension("png").to_string_lossy().as_ref());
        Ok(Value::Null)
    })
}

/// Write the PNG. A failure says so and changes nothing.
pub fn export_to(app: &mut PhotocraftApp, path: &str) {
    match app.run("file.export.quickExport", json!({"path": path})) {
        Ok(_) => {
            if let Some(st) = app.session.active_mut() {
                st.saved_revision = st.revision;
            }
            notices::post(app, tl!("PNG exported."), vec![path.to_string()], false, None);
        }
        Err(e) => {
            log::warn!("couldn't export {path}: {e}");
            notices::error(app, tl!("Couldn't save the PNG. Check that the folder can be written to, then try again.").to_string());
        }
    }
}

/// Run a menu command from the bar. Failures show like any other.
fn invoke(app: &mut PhotocraftApp, ctx: &egui::Context, id: &str) {
    if let Err(e) = menus::invoke(app, ctx, id, json!({})) {
        log::warn!("{id}: {e}");
    }
}

/// The bar: everything the app can do.
fn bar(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    egui::Panel::top("simple_bar").exact_size(44.0).frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(10, 0))).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            let ctx = ui.ctx().clone();
            let has_doc = app.session.active().is_some();
            if ui.button(tl!("Open Image…")).clicked() {
                invoke(app, &ctx, "file.open");
            }
            ui.separator();
            ui.add_enabled_ui(has_doc, |ui| {
                for (tool, label) in [(Tool::Brush, tl!("Pen")), (Tool::Eraser, tl!("Eraser"))] {
                    if ui.selectable_label(app.ui.tool == tool, label).clicked() {
                        app.ui.tool = tool;
                    }
                }
                ui.separator();
                color_button(app, ui);
                size_slider(app, ui);
                ui.separator();
                let (undo, redo) = app.session.active().map_or((false, false), |d| (d.history.can_undo(), d.history.can_redo()));
                if ui.add_enabled(undo, egui::Button::new(tl!("Undo"))).clicked() {
                    invoke(app, &ctx, "edit.undo");
                }
                if ui.add_enabled(redo, egui::Button::new(tl!("Redo"))).clicked() {
                    invoke(app, &ctx, "edit.redo");
                }
                ui.separator();
                if ui.button("−").on_hover_text(tl!("Zoom Out")).clicked() {
                    invoke(app, &ctx, "view.zoomOut");
                }
                ui.label(format!("{:.0}%", app.point_zoom() * 100.0));
                if ui.button("+").on_hover_text(tl!("Zoom In")).clicked() {
                    invoke(app, &ctx, "view.zoomIn");
                }
                if ui.button(tl!("Fit")).clicked() {
                    invoke(app, &ctx, "view.fitOnScreen");
                }
                if ui.selectable_label(app.ui.tool == Tool::Hand, tl!("Move View")).clicked() {
                    app.ui.tool = Tool::Hand;
                }
                ui.separator();
                if ui.button(tl!("Export PNG…")).clicked() {
                    invoke(app, &ctx, "file.export.quickExportAsPng");
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // The document's name is in the window title; here, the app's, which always fits.
                ui.label(egui::RichText::new(crate::app_name::app_name()).color(t.text_faint));
            });
        });
    });
}

fn color_button(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    ui.label(tl!("Color"));
    let c = app.session.tools.foreground;
    let to_byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut rgb = [to_byte(c[0]), to_byte(c[1]), to_byte(c[2])];
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        let hex = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
        if let Err(e) = app.run("tools.setColors", json!({"foreground": hex})) {
            log::warn!("{e}");
        }
    }
}

fn size_slider(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let label = ui.label(tl!("Thickness"));
    let mut size = app.session.tools.brush.size;
    let slider = ui.add(egui::Slider::new(&mut size, 1.0..=300.0).logarithmic(true).integer().show_value(false)).labelled_by(label.id);
    ui.label(format!("{size:.0} px"));
    if slider.changed()
        && let Err(e) = app.run("tools.setBrush", json!({"brush": {"size": size}}))
    {
        log::warn!("{e}");
    }
}

/// With no image yet, the one thing to do.
fn empty_screen(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() / 2.0 - 40.0).max(0.0));
            if ui.add(egui::Button::new(egui::RichText::new(tl!("Open Image…")).size(16.0)).min_size(egui::vec2(180.0, 40.0))).clicked() {
                invoke(app, &ctx, "file.open");
            }
            ui.add_space(10.0);
            ui.label(tl!("Or drop an image here."));
        });
    });
}

/// A new image arrived while the old one has a drawing not yet exported: which stays?
fn replace_question(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(old) = replacing(app) else { return };
    let Some(active) = app.session.active_index() else { return };
    let name = app.session.documents().get(old).map(|d| d.doc.name.clone()).unwrap_or_default();
    let mut close = None;
    egui::Modal::new(egui::Id::unique("simple-replace")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.label(i18n::fmt(tl!("The drawing on “{name}” has not been exported. Opening the new image discards it."), &[("name", &name)]));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(tl!("Discard Drawing")).clicked() {
                close = Some(old);
            }
            if ui.button(tl!("Keep Drawing")).clicked() {
                close = Some(active);
            }
        });
    });
    if let Some(i) = close {
        app.session.close(i);
        app.sync_views();
    }
}

/// The frame's UI between the window's edges.
pub fn body(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    bar(app, ui);
    let ctx = ui.ctx().clone();
    let t = crate::theme::Tokens::get(&ctx);
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.canvas)).show(ui, |ui| {
        if app.session.active().is_some() {
            canvas::document_area(app, ui);
        } else {
            empty_screen(app, ui);
        }
    });
    replace_question(app, &ctx);
    tick_hold(app, ctx.input(|i| i.time) * 1000.0);
    // A held pointer sends no events: keep frames coming to see the hold end.
    if app.drag.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests;
