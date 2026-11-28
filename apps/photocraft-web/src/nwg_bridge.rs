//! NWG (Will 25/11): bridge for the order-review page. `window.__nwgXuatOra()` returns a Promise of
//! `{ ten, bytes }`: the active document exported as OpenRaster (`ten` = its file name, `bytes` = a
//! Uint8Array), so the page's own "Done" button can send the edited layers back. The export runs on
//! the next frame, on the app's own state; with no document open the Promise rejects.
//!
//! NWG (Will 27/11 "layer chữ … đổi lại thành layer text để DS chỉnh sửa"): `window.__nwgChuSua(fonts, specs)`
//! turns the order's text layers (rasters the Mac drew) into editable type layers. `fonts` = an Array of
//! font files (Uint8Array; index 0 = the SKU's title font "script", 1 = its name font "serif"), `specs` =
//! JSON text, an array of `{id, chu, font, size_pt, mau:[r,g,b], x, y, h_scale, v_scale, leading_pt}`
//! (`id` = the `#id` suffix of the raster layer's name; `x, y` = centre / first baseline, document px;
//! `font` = "script" | "serif" | a family name). Each raster is replaced in place by a centred point type
//! layer with the same name, so the Mac still finds it by `#id` when it composes the saved file.
//! Resolves `{doi, loi}` (layers converted, problems). `__nwgXuatOra` also returns `chu`: the same specs
//! read back from the document's type layers, so the page can keep them in the saved file.
//!
//! NWG (Will 28/11, camp Bông rK87d1Ej: names run bottom-to-top): a spec may also carry `goc` (degrees,
//! clockwise, as Free Transform's `rotate`) and `tracking` (1/1000 em). With `goc`, `x, y` is the ink
//! centre of the rotated layer: the layer is created, turned about its centre, then moved so its ink
//! centre lands there. `text_specs` reads `goc` back from the layer's transform the same way.

use std::cell::RefCell;

use wasm_bindgen::{JsCast, JsValue, closure::Closure};

type Waiting = Vec<(js_sys::Function, js_sys::Function)>;

/// A `__nwgChuSua` call waiting for the next frame: (font files, specs JSON, resolve, reject).
type ChuWaiting = Vec<(Vec<Vec<u8>>, String, js_sys::Function, js_sys::Function)>;

thread_local! {
    static WAITING: RefCell<Waiting> = const { RefCell::new(Vec::new()) };
    static CHU: RefCell<ChuWaiting> = const { RefCell::new(Vec::new()) };
}

/// Puts `window.__nwgXuatOra` on the page. Called once, when the app is built.
pub(crate) fn install(ctx: egui::Context) {
    let Some(window) = web_sys::window() else { return };
    let ctx2 = ctx.clone();
    let f = Closure::<dyn FnMut() -> js_sys::Promise>::new(move || {
        let ctx = ctx.clone();
        js_sys::Promise::new(&mut |ok, fail| {
            WAITING.with(|w| w.borrow_mut().push((ok, fail)));
            ctx.request_repaint();
        })
    });
    let _ = js_sys::Reflect::set(&window, &JsValue::from_str("__nwgXuatOra"), f.as_ref());
    f.forget();
    let g = Closure::<dyn FnMut(JsValue, JsValue) -> js_sys::Promise>::new(move |fonts: JsValue, specs: JsValue| {
        let ctx = ctx2.clone();
        let files: Vec<Vec<u8>> = js_sys::Array::from(&fonts).iter().map(|v| if v.is_instance_of::<js_sys::Uint8Array>() { js_sys::Uint8Array::new(&v).to_vec() } else { Vec::new() }).collect();
        let specs = specs.as_string().unwrap_or_default();
        js_sys::Promise::new(&mut |ok, fail| {
            CHU.with(|w| w.borrow_mut().push((files.clone(), specs.clone(), ok, fail)));
            ctx.request_repaint();
        })
    });
    let _ = js_sys::Reflect::set(&window, &JsValue::from_str("__nwgChuSua"), g.as_ref());
    g.forget();
}

/// Registers the SKU's fonts; returns the family each file adds (empty when it adds none).
fn register_fonts(files: Vec<Vec<u8>>) -> Vec<String> {
    let mut eng = photocraft_text::shared().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    files.into_iter().map(|b| if b.is_empty() { String::new() } else { eng.fonts.register_font_data(b).into_iter().next().unwrap_or_default() }).collect()
}

/// The type.create parameters for one spec (pure, so it is tested without a browser).
pub(crate) fn create_params(spec: &serde_json::Value, families: &[String], name: &str) -> Result<serde_json::Value, String> {
    use serde_json::{Value, json};
    let num = |k: &str| spec.get(k).and_then(Value::as_f64).filter(|v| v.is_finite());
    let text = spec.get("chu").and_then(Value::as_str).filter(|t| !t.trim().is_empty()).ok_or("no text")?;
    let family = match spec.get("font").and_then(Value::as_str).unwrap_or_default() {
        "script" => families.first().cloned().unwrap_or_default(),
        "serif" => families.get(1).cloned().unwrap_or_default(),
        f => f.to_string(),
    };
    let size = num("size_pt").filter(|v| *v > 0.0).ok_or("no size")?;
    let (x, y) = (num("x").ok_or("no x")?, num("y").ok_or("no y")?);
    let mau = spec.get("mau").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).map(|c| (c / 255.0).clamp(0.0, 1.0)).collect::<Vec<_>>()).unwrap_or_default();
    let color = if mau.len() >= 3 { json!([mau[0], mau[1], mau[2], 1.0]) } else { json!([0.0, 0.0, 0.0, 1.0]) };
    let mut p = json!({"x": x, "y": y, "text": text.replace("\r\n", "\r").replace('\n', "\r"), "name": name, "align": "center", "size": size.min(1296.0), "color": color,
        "horizontalScale": num("h_scale").unwrap_or(100.0).clamp(1.0, 1000.0), "verticalScale": num("v_scale").unwrap_or(100.0).clamp(1.0, 1000.0)});
    if !family.is_empty() {
        p["font"] = json!(family);
    }
    if let Some(l) = num("leading_pt").filter(|v| *v > 0.0) {
        p["leading"] = json!(l);
    }
    if let Some(t) = num("tracking").filter(|v| *v != 0.0) {
        p["tracking"] = json!(t.clamp(-1000.0, 10000.0));
    }
    Ok(p)
}

/// The spec's rotation in degrees (clockwise), when it is a real turn.
fn spec_angle(spec: &serde_json::Value) -> Option<f64> {
    spec.get("goc").and_then(serde_json::Value::as_f64).filter(|g| g.is_finite() && g.abs() >= 0.05)
}

/// Ink box of a type layer in document pixels: (x0, y0, x1, y1).
fn ink_box(app: &photocraft_ui_egui::PhotocraftApp, id: u64) -> Option<(f64, f64, f64, f64)> {
    let st = app.session.active()?;
    let photocraft_doc::LayerContent::Text(t) = &st.doc.layer(photocraft_doc::LayerId(id))?.content else { return None };
    let b = t.cache.as_ref()?.content_bounds();
    (b.x1 > b.x0 && b.y1 > b.y0).then(|| (f64::from(b.x0), f64::from(b.y0), f64::from(b.x1), f64::from(b.y1)))
}

/// The document's layers whose name ends with ` #<id>`: (layer id, name), first match.
fn layer_by_tag(doc: &photocraft_doc::Document, tag: &str) -> Option<(u64, String)> {
    let suffix = format!(" #{tag}");
    doc.walk().into_iter().find(|(_, _, l)| l.name.ends_with(&suffix)).map(|(_, _, l)| (l.id.0, l.name.clone()))
}

fn convert(app: &mut photocraft_ui_egui::PhotocraftApp, files: Vec<Vec<u8>>, specs: &str) -> Result<JsValue, String> {
    use serde_json::{Value, json};
    let specs: Vec<Value> = serde_json::from_str(specs).map_err(|e| format!("specs: {e}"))?;
    if app.session.active().is_none() {
        return Err("no document is open".into());
    }
    let families = register_fonts(files);
    let mut doi = 0;
    let mut loi = Vec::new();
    for spec in &specs {
        let tag = spec.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
        let Some((raster, name)) = app.session.active().and_then(|st| layer_by_tag(&st.doc, &tag)) else {
            loi.push(format!("{tag}: no layer"));
            continue;
        };
        let is_raster = app.session.active().and_then(|st| st.doc.layer(photocraft_doc::LayerId(raster))).is_some_and(|l| matches!(l.content, photocraft_doc::LayerContent::Raster(_)));
        if !is_raster {
            continue; // already a type layer (reopened after a save)
        }
        let r = create_params(spec, &families, &name).and_then(|p| {
            app.run("layer.select", json!({"layer": raster, "mode": "replace"}))?;
            let visible = app.session.active().and_then(|st| st.doc.layer(photocraft_doc::LayerId(raster))).is_none_or(|l| l.visible);
            let want_x = p["x"].as_f64().unwrap_or_default();
            let want_y = p["y"].as_f64().unwrap_or_default();
            let made = app.run("type.create", p)?;
            let id = made["layer"].as_u64().ok_or("type.create returned no layer")?;
            if let Some(goc) = spec_angle(spec) {
                // Turned text (camp Bông names): rotate about the layer's centre, then put its ink centre on (x, y).
                app.run("edit.transform", json!({"layer": id, "rotate": goc, "reference": "center"}))?;
                if let Some((x0, y0, x1, y1)) = ink_box(app, id) {
                    let (dx, dy) = (want_x - (x0 + x1) / 2.0, want_y - (y0 + y1) / 2.0);
                    if dx.abs() >= 0.5 || dy.abs() >= 0.5 {
                        app.run("layer.translate", json!({"dx": dx, "dy": dy}))?;
                    }
                }
            } else if let Some(b) = made["bounds"].as_array().filter(|b| b.len() == 4) {
                // The engine centres the unscaled line, so a horizontal scale shifts the ink: the spec's
                // x is the ink centre, so move the new layer until its ink is centred there.
                let mid = (b[0].as_f64().unwrap_or_default() + b[2].as_f64().unwrap_or_default()) / 2.0;
                if (want_x - mid).abs() >= 0.5 {
                    app.run("layer.translate", json!({"dx": want_x - mid, "dy": 0.0}))?;
                }
            }
            if !visible {
                app.run("layer.setProps", json!({"layer": id, "visible": false}))?;
            }
            app.run("layer.delete", json!({"layer": raster}))?;
            app.run("layer.select", json!({"layer": id, "mode": "replace"}))?;
            Ok(())
        });
        match r {
            Ok(()) => doi += 1,
            Err(e) => loi.push(format!("{tag}: {e}")),
        }
    }
    let o = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&o, &JsValue::from_str("doi"), &JsValue::from_f64(f64::from(doi)));
    let _ = js_sys::Reflect::set(&o, &JsValue::from_str("loi"), &JsValue::from_str(&serde_json::to_string(&loi).unwrap_or_default()));
    let _ = js_sys::Reflect::set(&o, &JsValue::from_str("font"), &JsValue::from_str(&serde_json::to_string(&families).unwrap_or_default()));
    Ok(o.into())
}

/// The specs of the document's type layers whose name carries a `#id` (see the module doc).
pub(crate) fn text_specs(doc: &photocraft_doc::Document) -> serde_json::Value {
    use serde_json::json;
    let out: Vec<serde_json::Value> = doc
        .walk()
        .into_iter()
        .filter_map(|(_, _, l)| {
            let photocraft_doc::LayerContent::Text(t) = &l.content else { return None };
            let tag = l.name.rsplit_once(" #").map(|(_, t)| t.to_string())?;
            let run = t.char_runs().into_iter().next().map(|r| r.style);
            let c = t.color.to_rgb();
            let m = t.transform.m;
            let goc = m[1].atan2(m[0]).to_degrees();
            let ink = t.cache.as_ref().map(|c| c.content_bounds()).filter(|b| b.x1 > b.x0 && b.y1 > b.y0);
            if goc.abs() >= 0.05 {
                // Turned layer: x, y = ink centre, plus the angle (see the module doc).
                let (x, y) = ink.map_or((m[4], m[5]), |b| ((f64::from(b.x0) + f64::from(b.x1)) / 2.0, (f64::from(b.y0) + f64::from(b.y1)) / 2.0));
                return Some(json!({"id": tag, "chu": t.text.replace('\r', "\n"), "font": t.font_family, "size_pt": t.size_pt,
                    "mau": [(c[0] * 255.0).round(), (c[1] * 255.0).round(), (c[2] * 255.0).round()], "x": x, "y": y, "goc": (goc * 100.0).round() / 100.0,
                    "h_scale": run.as_ref().map_or(100.0, |r| r.horizontal_scale * 100.0), "v_scale": run.as_ref().map_or(100.0, |r| r.vertical_scale * 100.0),
                    "leading_pt": run.as_ref().and_then(|r| r.leading_pt), "tracking": run.as_ref().map(|r| r.tracking)}));
            }
            Some(json!({"id": tag, "chu": t.text.replace('\r', "\n"), "font": t.font_family, "size_pt": t.size_pt,
                "mau": [(c[0] * 255.0).round(), (c[1] * 255.0).round(), (c[2] * 255.0).round()],
                // x = ink centre (what create_params + the recentring above expect); y = baseline.
                "x": t.cache.as_ref().map(|c| c.content_bounds()).filter(|b| b.x1 > b.x0).map_or(t.transform.m[4], |b| (f64::from(b.x0) + f64::from(b.x1)) / 2.0), "y": t.transform.m[5],
                // The engine stores scales as fractions (0.8); the spec carries percent (80).
                "h_scale": run.as_ref().map_or(100.0, |r| r.horizontal_scale * 100.0), "v_scale": run.as_ref().map_or(100.0, |r| r.vertical_scale * 100.0),
                "leading_pt": run.as_ref().and_then(|r| r.leading_pt), "tracking": run.as_ref().map(|r| r.tracking)}))
        })
        .collect();
    serde_json::Value::Array(out)
}

/// Answers the waiting calls from the app's state (each frame; nothing to do almost always).
pub(crate) fn serve(app: &mut photocraft_ui_egui::PhotocraftApp) {
    let chu = CHU.with(|w| std::mem::take(&mut *w.borrow_mut()));
    for (files, specs, ok, fail) in chu {
        let _ = match convert(app, files, &specs) {
            Ok(v) => ok.call1(&JsValue::NULL, &v),
            Err(e) => fail.call1(&JsValue::NULL, &JsValue::from_str(&e)),
        };
    }
    let waiting = WAITING.with(|w| std::mem::take(&mut *w.borrow_mut()));
    if waiting.is_empty() {
        return;
    }
    let result: Result<JsValue, String> = match app.session.active() {
        None => Err("no document is open".into()),
        Some(st) => photocraft_io::export(&st.doc, "ora", &photocraft_io::ExportOptions::default()).map_err(|e| e.to_string()).map(|r| {
            let name = st
                .path
                .as_deref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .map_or_else(|| "document.ora".to_string(), |n| n.to_string_lossy().to_string());
            let o = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&o, &JsValue::from_str("ten"), &JsValue::from_str(&name));
            let _ = js_sys::Reflect::set(&o, &JsValue::from_str("bytes"), &js_sys::Uint8Array::from(r.bytes.as_slice()));
            let _ = js_sys::Reflect::set(&o, &JsValue::from_str("chu"), &JsValue::from_str(&text_specs(&st.doc).to_string()));
            o.into()
        }),
    };
    for (ok, fail) in waiting {
        let _ = match &result {
            Ok(v) => ok.call1(&JsValue::NULL, v),
            Err(e) => fail.call1(&JsValue::NULL, &JsValue::from_str(e)),
        };
    }
}
