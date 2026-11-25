//! NWG (Will 25/11): bridge for the order-review page. `window.__nwgXuatOra()` returns a Promise of
//! `{ ten, bytes }`: the active document exported as OpenRaster (`ten` = its file name, `bytes` = a
//! Uint8Array), so the page's own "Done" button can send the edited layers back. The export runs on
//! the next frame, on the app's own state; with no document open the Promise rejects.

use std::cell::RefCell;

use wasm_bindgen::{JsValue, closure::Closure};

type Waiting = Vec<(js_sys::Function, js_sys::Function)>;

thread_local! {
    static WAITING: RefCell<Waiting> = const { RefCell::new(Vec::new()) };
}

/// Puts `window.__nwgXuatOra` on the page. Called once, when the app is built.
pub(crate) fn install(ctx: egui::Context) {
    let Some(window) = web_sys::window() else { return };
    let f = Closure::<dyn FnMut() -> js_sys::Promise>::new(move || {
        let ctx = ctx.clone();
        js_sys::Promise::new(&mut |ok, fail| {
            WAITING.with(|w| w.borrow_mut().push((ok, fail)));
            ctx.request_repaint();
        })
    });
    let _ = js_sys::Reflect::set(&window, &JsValue::from_str("__nwgXuatOra"), f.as_ref());
    f.forget();
}

/// Answers the waiting calls from the app's state (each frame; nothing to do almost always).
pub(crate) fn serve(app: &photocraft_ui_egui::PhotocraftApp) {
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
