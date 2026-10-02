//! overlay.rs — SuperWhisper-style floating "pill" shown during dictation.
//!
//! NATIVE Win32, zero WebView2. SuperWhisper is light precisely because its
//! overlay is a native window (AppKit on macOS, Win32 on Windows) — never a
//! browser engine. We do the same: a layered (`WS_EX_LAYERED`) top-most popup
//! drawn with per-pixel alpha (the rounded translucent pill + animated
//! waveform), costing ~2-5 MB instead of the old WebView2's ~300 MB (which also
//! grew ~14 MB/s while shown). Being native it also dodges Tauri v2's
//! transparent-window recreate bug (#9307) that turned the WebView pill black —
//! we just `SW_HIDE` between dictations, no destroy/recreate needed.
//!
//! The window lives on its own dedicated thread with a classic Win32 message
//! loop + animation timer; `show`/`hide` only flip an atomic and post a wake
//! message, so they are safe to call from any Tokio/Tauri thread.
//!
//! Best-effort everywhere: failing to create or paint the overlay must NEVER
//! break the dictation pipeline. The tray icon stays the source-of-truth status.

use tauri::AppHandle;

/// Show the pill in `state` ("recording" | "transcribing").
pub fn show(_app: &AppHandle, state: &str) {
    #[cfg(windows)]
    win::show(state);
    #[cfg(not(windows))]
    let _ = state;
}

/// Hide the pill (native window is hidden, not destroyed — it is cheap).
pub fn hide(_app: &AppHandle) {
    #[cfg(windows)]
    win::hide();
}

/// Visual smoke test (no Tauri runtime needed): flash the pill through its
/// states so the native overlay can be eyeballed via the `overlay_test` bin.
#[cfg(windows)]
pub fn demo() {
    win::show("recording");
    std::thread::sleep(std::time::Duration::from_secs(4));
    win::show("transcribing");
    std::thread::sleep(std::time::Duration::from_secs(4));
    win::hide();
    std::thread::sleep(std::time::Duration::from_millis(400));
}

#[cfg(windows)]
mod win {
    use std::cell::RefCell;
    use std::sync::Once;
    use std::sync::atomic::{AtomicIsize, AtomicU8, Ordering};

    use windows::Win32::Foundation::{
        COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
    };
    use windows::Win32::Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER,
        BLENDFUNCTION, CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection, CreateFontW,
        DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DRAW_TEXT_FORMAT, DT_LEFT, DT_NOPREFIX,
        DT_SINGLELINE, DeleteDC, DeleteObject, DrawTextW, FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, HDC,
        HFONT, HGDIOBJ, OUT_DEFAULT_PRECIS, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::HiDpi::GetDpiForSystem;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetSystemMetrics, HMENU,
        IDC_ARROW, KillTimer, LoadCursorW, MSG, PostMessageW, RegisterClassExW, SM_CXSCREEN,
        SM_CYSCREEN, SW_HIDE, SW_SHOWNOACTIVATE, SetTimer, ShowWindow, TranslateMessage, ULW_ALPHA,
        UpdateLayeredWindow, WM_APP, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };
    use windows::core::{PCWSTR, w};

    // ── Shared state between the caller threads and the overlay thread ──────────
    /// 0 = hidden, 1 = recording, 2 = transcribing.
    static STATE: AtomicU8 = AtomicU8::new(0);
    /// The overlay HWND as an isize (0 until the window thread has created it).
    static HWND_RAW: AtomicIsize = AtomicIsize::new(0);
    static INIT: Once = Once::new();

    /// Custom "re-read STATE and apply" message posted to wake the message loop.
    const WM_OVL_REFRESH: u32 = WM_APP + 1;
    /// Animation timer id + period (~30 fps).
    const TIMER_ID: usize = 1;
    const TIMER_MS: u32 = 33;

    /// Logical (DPI-independent) panel geometry — scaled up at create time.
    const W: f64 = 360.0;
    const H: f64 = 96.0;
    const RADIUS: f64 = 16.0;
    const BOTTOM_GAP: f64 = 56.0;
    const FOOTER_H: f64 = 27.0;
    const BAR_COUNT: usize = 48;

    pub(super) fn show(state: &str) {
        let code = match state {
            "transcribing" => 2u8,
            _ => 1u8,
        };
        STATE.store(code, Ordering::SeqCst);
        INIT.call_once(|| {
            if let Err(e) = std::thread::Builder::new()
                .name("overlay".into())
                .spawn(thread_main)
            {
                tracing::warn!("overlay thread spawn failed: {e}");
            }
        });
        wake();
    }

    pub(super) fn hide() {
        STATE.store(0, Ordering::SeqCst);
        wake();
    }

    /// Post the refresh message if the window already exists (otherwise the
    /// freshly-spawned thread will read STATE itself right after creating it).
    fn wake() {
        let raw = HWND_RAW.load(Ordering::SeqCst);
        if raw != 0 {
            // SAFETY: `raw` is a valid HWND created by the overlay thread; it is
            // only zeroed before creation, which we guarded against above.
            // PostMessageW is documented as safe to call cross-thread.
            unsafe {
                let _ = PostMessageW(HWND(raw as *mut _), WM_OVL_REFRESH, WPARAM(0), LPARAM(0));
            }
        }
    }

    // ── The overlay thread: owns the window + message loop ──────────────────────
    thread_local! {
        static RENDERER: RefCell<Option<Renderer>> = const { RefCell::new(None) };
    }

    fn thread_main() {
        // SAFETY: standard Win32 window bootstrap. Every call is checked; on any
        // failure we log and bail so the dictation pipeline is unaffected.
        unsafe {
            let hmodule = match GetModuleHandleW(PCWSTR::null()) {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!("overlay GetModuleHandle failed: {e}");
                    return;
                }
            };
            let hinstance = HINSTANCE(hmodule.0);
            let class_name = w!("SuperParlerOverlay");
            let cursor = LoadCursorW(HINSTANCE::default(), IDC_ARROW).unwrap_or_default();
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance,
                hCursor: cursor,
                lpszClassName: class_name,
                ..Default::default()
            };
            if RegisterClassExW(&wc) == 0 {
                tracing::warn!("overlay RegisterClassExW failed");
                return;
            }

            let scale = (GetDpiForSystem() as f64 / 96.0).max(1.0);
            let pw = (W * scale).round() as i32;
            let ph = (H * scale).round() as i32;
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let screen_h = GetSystemMetrics(SM_CYSCREEN);
            let x = (screen_w - pw) / 2;
            let y = screen_h - ph - (BOTTOM_GAP * scale).round() as i32;

            let hwnd = match CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                class_name,
                w!("SuperParler"),
                WS_POPUP,
                x,
                y,
                pw,
                ph,
                HWND::default(),
                HMENU::default(),
                hinstance,
                None,
            ) {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!("overlay CreateWindowExW failed: {e}");
                    return;
                }
            };

            match Renderer::new(hwnd, pw, ph, x, y, scale) {
                Some(r) => RENDERER.with(|cell| *cell.borrow_mut() = Some(r)),
                None => {
                    tracing::warn!("overlay renderer init failed");
                    return;
                }
            }

            HWND_RAW.store(hwnd.0 as isize, Ordering::SeqCst);
            tracing::info!("native overlay window created");
            apply(hwnd); // honour whatever STATE was set before we existed

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_OVL_REFRESH => {
                apply(hwnd);
                LRESULT(0)
            }
            WM_TIMER => {
                if STATE.load(Ordering::SeqCst) != 0 {
                    RENDERER.with(|cell| {
                        if let Some(r) = cell.borrow_mut().as_mut() {
                            r.paint();
                        }
                    });
                }
                LRESULT(0)
            }
            // SAFETY: defer everything else to the default handler.
            _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
        }
    }

    /// Read STATE and show/hide + (re)arm the animation timer accordingly.
    fn apply(hwnd: HWND) {
        let visible = STATE.load(Ordering::SeqCst) != 0;
        // SAFETY: `hwnd` is the live overlay window owned by this thread.
        unsafe {
            if visible {
                RENDERER.with(|cell| {
                    if let Some(r) = cell.borrow_mut().as_mut() {
                        r.paint();
                    }
                });
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                SetTimer(hwnd, TIMER_ID, TIMER_MS, None);
            } else {
                let _ = KillTimer(hwnd, TIMER_ID);
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }

    // ── Renderer: owns the 32-bpp DIB we composite into and blit each frame ─────
    struct Renderer {
        hwnd: HWND,
        w: i32,
        h: i32,
        x: i32,
        y: i32,
        scale: f64,
        mem_dc: HDC,
        dib: windows::Win32::Graphics::Gdi::HBITMAP,
        bits: *mut u32,
        // Scratch DC/DIB used only to rasterize anti-aliased text, whose grey
        // coverage we then composite into `bits` with proper alpha.
        text_dc: HDC,
        text_dib: windows::Win32::Graphics::Gdi::HBITMAP,
        text_bits: *mut u32,
        font_label: HFONT,
        font_kbd: HFONT,
        /// Smoothed per-bar levels (0..1), driven by the live voice spectrum.
        levels: [f32; BAR_COUNT],
    }

    impl Renderer {
        fn new(hwnd: HWND, w: i32, h: i32, x: i32, y: i32, scale: f64) -> Option<Self> {
            // SAFETY: GDI object creation; each handle is checked for validity.
            unsafe {
                let (mem_dc, dib, bits) = make_dib(w, h)?;
                let (text_dc, text_dib, text_bits) = make_dib(w, h)?;
                let px = |p: f64| -(((p * scale).round()) as i32); // negative = char height
                let font_label = CreateFontW(
                    px(12.0),
                    0,
                    0,
                    0,
                    FW_NORMAL.0 as i32,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET.0 as u32,
                    OUT_DEFAULT_PRECIS.0 as u32,
                    CLIP_DEFAULT_PRECIS.0 as u32,
                    ANTIALIASED_QUALITY.0 as u32,
                    (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                    w!("Segoe UI"),
                );
                let font_kbd = CreateFontW(
                    px(10.5),
                    0,
                    0,
                    0,
                    FW_SEMIBOLD.0 as i32,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET.0 as u32,
                    OUT_DEFAULT_PRECIS.0 as u32,
                    CLIP_DEFAULT_PRECIS.0 as u32,
                    ANTIALIASED_QUALITY.0 as u32,
                    (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                    w!("Segoe UI"),
                );
                if font_label.is_invalid() || font_kbd.is_invalid() {
                    return None;
                }
                Some(Renderer {
                    hwnd,
                    w,
                    h,
                    x,
                    y,
                    scale,
                    mem_dc,
                    dib,
                    bits,
                    text_dc,
                    text_dib,
                    text_bits,
                    font_label,
                    font_kbd,
                    levels: [0.0; BAR_COUNT],
                })
            }
        }

        fn buf(&mut self) -> &mut [u32] {
            // SAFETY: `bits` points at w*h u32s owned by our DIB section.
            unsafe { std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize) }
        }

        fn paint(&mut self) {
            let state = STATE.load(Ordering::SeqCst);
            if state == 0 {
                return;
            }
            let busy = state == 2;
            self.update_levels(busy);
            self.draw_panel(busy);
            self.draw_waveform(busy);
            self.draw_footer(busy);
            self.blit();
        }

        /// Pull the live voice spectrum into smoothed per-bar levels. Fast attack,
        /// slow release so the bars feel responsive but not jittery. While
        /// transcribing (no live mic) or before enough audio, the bars decay.
        fn update_levels(&mut self, busy: bool) {
            if busy {
                for l in &mut self.levels {
                    *l *= 0.90;
                }
                return;
            }
            let mut raw = [0f32; BAR_COUNT];
            if crate::spectrum::compute_bands(&mut raw) {
                for (l, &target) in self.levels.iter_mut().zip(raw.iter()) {
                    let k = if target > *l { 0.6 } else { 0.25 };
                    *l += (target - *l) * k;
                }
            } else {
                for l in &mut self.levels {
                    *l *= 0.85;
                }
            }
        }

        /// Rounded gradient panel — written as the opaque base layer.
        fn draw_panel(&mut self, _busy: bool) {
            let (w, h, s) = (self.w as f64, self.h as f64, self.scale);
            let (iw, ih) = (self.w, self.h);
            let r = RADIUS * s;
            let buf = self.buf();
            for py in 0..ih {
                for px in 0..iw {
                    let cov = round_rect_cov(px as f64 + 0.5, py as f64 + 0.5, w, h, r);
                    let idx = (py * iw + px) as usize;
                    if cov <= 0.0 {
                        buf[idx] = 0;
                        continue;
                    }
                    // Vertical gradient #2a2a30 → #161619.
                    let t = (py as f64 / h).clamp(0.0, 1.0);
                    let cr = lerp(0x2a, 0x16, t);
                    let cg = lerp(0x2a, 0x16, t);
                    let cb = lerp(0x30, 0x19, t);
                    buf[idx] = pack(cov as f32, cr, cg, cb);
                    // Subtle inner border (white @ ~8%) within ~1.4px of the edge.
                    let sd = -round_rect_sd(px as f64 + 0.5, py as f64 + 0.5, w, h, r);
                    if sd < 1.4 * s {
                        let edge = (1.0 - (sd / (1.4 * s))).clamp(0.0, 1.0) * 0.08;
                        buf[idx] = over(buf[idx], edge as f32, 255.0, 255.0, 255.0);
                    }
                }
            }
        }

        fn draw_waveform(&mut self, busy: bool) {
            let s = self.scale;
            let (cr, cg, cb) = if busy {
                (255.0, 179.0, 61.0)
            } else {
                (236.0, 236.0, 242.0)
            };
            let pad_x = 16.0 * s;
            let area_top = 8.0 * s;
            let area_bot = (H - FOOTER_H - 6.0) * s;
            let area_w = self.w as f64 - 2.0 * pad_x;
            let mid = (area_top + area_bot) / 2.0;
            let area_h = area_bot - area_top;
            let max_bar = area_h.min(34.0 * s);
            let bar_w = (2.5 * s).max(1.0);
            let gap = area_w / BAR_COUNT as f64;
            let base = 3.0 * s;
            let (w, h) = (self.w, self.h);
            let levels = self.levels; // Copy: avoids borrowing self while buf() is held
            let buf = self.buf();
            for (i, &level) in levels.iter().enumerate() {
                // Mild center emphasis (taper) caps edge bars so they don't slam the
                // rounded panel — but the HEIGHT is driven by the real band energy,
                // so the bars follow the voice's tonality (low bars = bass, high = treble).
                let tt = i as f64 / (BAR_COUNT - 1) as f64;
                let taper = (tt * std::f64::consts::PI).sin();
                let cap = base + (max_bar - base) * (0.5 + 0.5 * taper);
                let bh = base + (cap - base) * level as f64;
                let bx = pad_x + i as f64 * gap + (gap - bar_w) / 2.0;
                fill_round_bar(buf, w, h, bx, mid - bh / 2.0, bar_w, bh, cr, cg, cb, 0.92);
            }
        }

        fn draw_footer(&mut self, busy: bool) {
            let s = self.scale;
            let footer_top = ((H - FOOTER_H) * s).round() as i32;
            let w = self.w;
            let h = self.h;
            {
                let buf = self.buf();
                // Footer band (black @ 18%) + 1px top separator (white @ 6%).
                for py in footer_top..h {
                    for px in 0..w {
                        let idx = (py * w + px) as usize;
                        if (buf[idx] >> 24) == 0 {
                            continue;
                        }
                        buf[idx] = over(buf[idx], 0.18, 0.0, 0.0, 0.0);
                        if py == footer_top {
                            buf[idx] = over(buf[idx], 0.06, 255.0, 255.0, 255.0);
                        }
                    }
                }
            }
            // Mic glyph (red / amber) on the left of the footer.
            let (mr, mg, mb) = if busy {
                (255.0, 179.0, 61.0)
            } else {
                (255.0, 91.0, 91.0)
            };
            let mic_cx = 18.0 * s;
            let mic_cy = (H - FOOTER_H / 2.0) * s;
            self.draw_mic(mic_cx, mic_cy, mr, mg, mb);

            // Label text.
            let label = if busy {
                "Transcription…"
            } else {
                "Enregistrement…"
            };
            let lx = (31.0 * s).round() as i32;
            let label_rect = RECT {
                left: lx,
                top: footer_top,
                right: w,
                bottom: h,
            };
            self.draw_text(label, self.font_label, (139, 139, 149), label_rect, DT_LEFT);

            // Keyboard hint, right-aligned: Ctrl ⇧ Espace.
            self.draw_kbd(footer_top, h);
        }

        fn draw_mic(&mut self, cx: f64, cy: f64, r: f64, g: f64, b: f64) {
            let s = self.scale;
            let w = self.w;
            let h = self.h;
            let buf = self.buf();
            let body_w = 5.0 * s;
            let body_h = 8.0 * s;
            // Capsule body.
            fill_round_bar(
                buf,
                w,
                h,
                cx - body_w / 2.0,
                cy - body_h / 2.0 - 1.0 * s,
                body_w,
                body_h,
                r,
                g,
                b,
                1.0,
            );
            // Stand + base, drawn as thin bars.
            fill_round_bar(
                buf,
                w,
                h,
                cx - 0.8 * s,
                cy + body_h / 2.0 - 1.0 * s,
                1.6 * s,
                2.5 * s,
                r,
                g,
                b,
                1.0,
            );
            fill_round_bar(
                buf,
                w,
                h,
                cx - 3.0 * s,
                cy + body_h / 2.0 + 1.0 * s,
                6.0 * s,
                1.4 * s,
                r,
                g,
                b,
                1.0,
            );
        }

        fn draw_kbd(&mut self, footer_top: i32, bottom: i32) {
            let s = self.scale;
            let keys = ["Ctrl", "⇧", "Espace"];
            let pad = (6.0 * s).round() as i32;
            let key_gap = (4.0 * s).round() as i32;
            let kh = (16.0 * s).round() as i32;
            let mut widths = [0i32; 3];
            for (i, k) in keys.iter().enumerate() {
                widths[i] = self.text_width(k, self.font_kbd) + pad * 2;
            }
            let total: i32 = widths.iter().sum::<i32>() + key_gap * (keys.len() as i32 - 1);
            let mut kx = self.w - (12.0 * s).round() as i32 - total;
            let ky = (footer_top + bottom - kh) / 2;
            for (i, k) in keys.iter().enumerate() {
                let kw = widths[i];
                // Key background (white @ 8%) + border (white @ 8%).
                self.fill_round_rect(kx, ky, kw, kh, 5.0 * s, (255, 255, 255), 0.08);
                let rect = RECT {
                    left: kx,
                    top: ky,
                    right: kx + kw,
                    bottom: ky + kh,
                };
                self.draw_text_centered(k, self.font_kbd, (201, 201, 210), rect);
                kx += kw + key_gap;
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn fill_round_rect(
            &mut self,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            r: f64,
            c: (u8, u8, u8),
            a: f64,
        ) {
            let bw = self.w;
            let bh = self.h;
            let buf = self.buf();
            for py in y..(y + h) {
                for px in x..(x + w) {
                    if px < 0 || py < 0 || px >= bw || py >= bh {
                        continue;
                    }
                    let cov = round_rect_cov_at(
                        (px - x) as f64 + 0.5,
                        (py - y) as f64 + 0.5,
                        w as f64,
                        h as f64,
                        r,
                    );
                    if cov <= 0.0 {
                        continue;
                    }
                    let idx = (py * bw + px) as usize;
                    buf[idx] = over(
                        buf[idx],
                        (a * cov) as f32,
                        c.0 as f64,
                        c.1 as f64,
                        c.2 as f64,
                    );
                }
            }
        }

        // ── Text via GDI grey-coverage → alpha composite ────────────────────────
        fn text_width(&self, text: &str, font: HFONT) -> i32 {
            // SAFETY: text_dc is a valid memory DC; font is a valid HFONT.
            unsafe {
                let old = SelectObject(self.text_dc, HGDIOBJ(font.0));
                let mut wide: Vec<u16> = text.encode_utf16().collect();
                let mut r = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                // DT_CALCRECT (0x400) measures without drawing.
                DrawTextW(
                    self.text_dc,
                    &mut wide,
                    &mut r,
                    DRAW_TEXT_FORMAT(0x400) | DT_SINGLELINE | DT_NOPREFIX,
                );
                SelectObject(self.text_dc, old);
                r.right - r.left
            }
        }

        fn draw_text(
            &mut self,
            text: &str,
            font: HFONT,
            color: (u8, u8, u8),
            rect: RECT,
            fmt: DRAW_TEXT_FORMAT,
        ) {
            self.render_text(
                text,
                font,
                color,
                rect,
                fmt | DT_SINGLELINE | DT_NOPREFIX | DRAW_TEXT_FORMAT(0x4), /*DT_VCENTER*/
            );
        }

        fn draw_text_centered(&mut self, text: &str, font: HFONT, color: (u8, u8, u8), rect: RECT) {
            // DT_CENTER (1) | DT_VCENTER (4).
            self.render_text(
                text,
                font,
                color,
                rect,
                DRAW_TEXT_FORMAT(0x1) | DRAW_TEXT_FORMAT(0x4) | DT_SINGLELINE | DT_NOPREFIX,
            );
        }

        fn render_text(
            &mut self,
            text: &str,
            font: HFONT,
            color: (u8, u8, u8),
            rect: RECT,
            fmt: DRAW_TEXT_FORMAT,
        ) {
            let (w, h) = (self.w, self.h);
            let x0 = rect.left.max(0);
            let y0 = rect.top.max(0);
            let x1 = rect.right.min(w);
            let y1 = rect.bottom.min(h);
            if x1 <= x0 || y1 <= y0 {
                return;
            }
            // SAFETY: render white-on-black AA text into the scratch DIB, then
            // read its grey level as coverage. text_dc/text_bits are valid.
            unsafe {
                let tbuf = std::slice::from_raw_parts_mut(self.text_bits, (w * h) as usize);
                for py in y0..y1 {
                    for px in x0..x1 {
                        tbuf[(py * w + px) as usize] = 0xFF00_0000; // opaque black
                    }
                }
                let old = SelectObject(self.text_dc, HGDIOBJ(font.0));
                SetBkMode(self.text_dc, TRANSPARENT);
                SetTextColor(self.text_dc, COLORREF(0x00FF_FFFF)); // white
                let mut wide: Vec<u16> = text.encode_utf16().collect();
                let mut r = rect;
                DrawTextW(self.text_dc, &mut wide, &mut r, fmt);
                SelectObject(self.text_dc, old);

                let buf = std::slice::from_raw_parts_mut(self.bits, (w * h) as usize);
                for py in y0..y1 {
                    for px in x0..x1 {
                        let idx = (py * w + px) as usize;
                        let cov = (tbuf[idx] & 0xFF) as f32 / 255.0; // blue == grey level
                        if cov > 0.003 {
                            buf[idx] = over(
                                buf[idx],
                                cov,
                                color.0 as f64,
                                color.1 as f64,
                                color.2 as f64,
                            );
                        }
                    }
                }
            }
        }

        fn blit(&mut self) {
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let size = SIZE {
                cx: self.w,
                cy: self.h,
            };
            let src = POINT { x: 0, y: 0 };
            let dst = POINT {
                x: self.x,
                y: self.y,
            };
            // SAFETY: all handles/pointers are live for the duration of the call.
            unsafe {
                let _ = UpdateLayeredWindow(
                    self.hwnd,
                    HDC::default(),
                    Some(&dst),
                    Some(&size),
                    self.mem_dc,
                    Some(&src),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                );
            }
        }
    }

    impl Drop for Renderer {
        fn drop(&mut self) {
            // SAFETY: handles created in `new`; freed exactly once here.
            unsafe {
                let _ = DeleteObject(HGDIOBJ(self.font_label.0));
                let _ = DeleteObject(HGDIOBJ(self.font_kbd.0));
                let _ = DeleteObject(HGDIOBJ(self.dib.0));
                let _ = DeleteObject(HGDIOBJ(self.text_dib.0));
                let _ = DeleteDC(self.mem_dc);
                let _ = DeleteDC(self.text_dc);
            }
        }
    }

    /// Create a memory DC with a top-down 32-bpp DIB selected; return its bits.
    fn make_dib(w: i32, h: i32) -> Option<(HDC, windows::Win32::Graphics::Gdi::HBITMAP, *mut u32)> {
        // SAFETY: standard GDI DIB-section creation; the DC is freed on the
        // failure path and otherwise owned by the returned tuple (freed in Drop).
        unsafe {
            let dc = CreateCompatibleDC(HDC::default());
            if dc.is_invalid() {
                return None;
            }
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let dib = match CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(b) if !bits.is_null() => b,
                _ => {
                    let _ = DeleteDC(dc);
                    return None;
                }
            };
            SelectObject(dc, HGDIOBJ(dib.0));
            Some((dc, dib, bits as *mut u32))
        }
    }

    // ── Pure compositing helpers (premultiplied BGRA, packed 0xAARRGGBB) ────────
    fn lerp(a: i64, b: i64, t: f64) -> f64 {
        a as f64 + (b as f64 - a as f64) * t
    }

    /// Pack straight color + alpha into premultiplied 0xAARRGGBB.
    fn pack(a: f32, r: f64, g: f64, b: f64) -> u32 {
        let ai = (a * 255.0).round().clamp(0.0, 255.0) as u32;
        let rp = (r * a as f64).round().clamp(0.0, 255.0) as u32;
        let gp = (g * a as f64).round().clamp(0.0, 255.0) as u32;
        let bp = (b * a as f64).round().clamp(0.0, 255.0) as u32;
        (ai << 24) | (rp << 16) | (gp << 8) | bp
    }

    /// Source-over composite of straight color `(r,g,b)` with coverage `a` onto
    /// an existing premultiplied pixel.
    fn over(dst: u32, a: f32, r: f64, g: f64, b: f64) -> u32 {
        let a = a.clamp(0.0, 1.0) as f64;
        let da = ((dst >> 24) & 0xFF) as f64 / 255.0;
        let dr = ((dst >> 16) & 0xFF) as f64;
        let dg = ((dst >> 8) & 0xFF) as f64;
        let db = (dst & 0xFF) as f64;
        let oa = a + da * (1.0 - a);
        let or = r * a + dr * (1.0 - a);
        let og = g * a + dg * (1.0 - a);
        let ob = b * a + db * (1.0 - a);
        ((oa * 255.0).round().clamp(0.0, 255.0) as u32) << 24
            | (or.round().clamp(0.0, 255.0) as u32) << 16
            | (og.round().clamp(0.0, 255.0) as u32) << 8
            | (ob.round().clamp(0.0, 255.0) as u32)
    }

    /// Fill a small rounded vertical bar by alpha-compositing over `buf`.
    #[allow(clippy::too_many_arguments)]
    fn fill_round_bar(
        buf: &mut [u32],
        bw: i32,
        bh: i32,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        r: f64,
        g: f64,
        b: f64,
        a: f64,
    ) {
        let radius = (w / 2.0).min(h / 2.0);
        let x0 = x.floor().max(0.0) as i32;
        let y0 = y.floor().max(0.0) as i32;
        let x1 = (x + w).ceil().min(bw as f64) as i32;
        let y1 = (y + h).ceil().min(bh as f64) as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                let cov = round_rect_cov_at(px as f64 + 0.5 - x, py as f64 + 0.5 - y, w, h, radius);
                if cov <= 0.0 {
                    continue;
                }
                let idx = (py * bw + px) as usize;
                buf[idx] = over(buf[idx], (a * cov) as f32, r, g, b);
            }
        }
    }

    /// Signed distance to a rounded rect spanning the whole `w×h` window.
    fn round_rect_sd(px: f64, py: f64, w: f64, h: f64, r: f64) -> f64 {
        let cx = w / 2.0;
        let cy = h / 2.0;
        let dx = (px - cx).abs() - (cx - r);
        let dy = (py - cy).abs() - (cy - r);
        let ax = dx.max(0.0);
        let ay = dy.max(0.0);
        (ax * ax + ay * ay).sqrt() + dx.max(dy).min(0.0) - r
    }

    fn round_rect_cov(px: f64, py: f64, w: f64, h: f64, r: f64) -> f64 {
        (0.5 - round_rect_sd(px, py, w, h, r)).clamp(0.0, 1.0)
    }

    /// Same, but for a sub-rect placed at local origin (px,py already local).
    fn round_rect_cov_at(px: f64, py: f64, w: f64, h: f64, r: f64) -> f64 {
        let cx = w / 2.0;
        let cy = h / 2.0;
        let dx = (px - cx).abs() - (cx - r);
        let dy = (py - cy).abs() - (cy - r);
        let ax = dx.max(0.0);
        let ay = dy.max(0.0);
        let sd = (ax * ax + ay * ay).sqrt() + dx.max(dy).min(0.0) - r;
        (0.5 - sd).clamp(0.0, 1.0)
    }
}
