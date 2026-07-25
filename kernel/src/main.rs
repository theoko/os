#![no_std]
#![no_main]

use core::hint::black_box;

use kernel::{anim, beep, caps, fb, hello_message, keyboard, mcp, mouse, screens, searchui, serial, setup, skills, ui, usb_tablet};
use limine::BaseRevision;
use limine::request::{
    FramebufferRequest, HhdmRequest, MemoryMapRequest, RequestsEndMarker, RequestsStartMarker,
    StackSizeRequest,
};

const STACK_SIZE: u64 = 128 * 1024;

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".requests")]
static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(STACK_SIZE);

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

#[unsafe(no_mangle)]
unsafe extern "C" fn kmain() -> ! {
    black_box(&BASE_REVISION);
    black_box(&STACK_SIZE_REQUEST);
    black_box(&HHDM_REQUEST);
    black_box(&MEMORY_MAP_REQUEST);
    black_box(&FRAMEBUFFER_REQUEST);

    if !BASE_REVISION.is_supported() {
        serial::exit_qemu(false);
    }

    let serial_port = serial::Serial::com1();
    serial_port.init();
    serial_port.write_str(hello_message());
    serial_port.write_str(serial::LINE_ENDING);

    // Paint UI immediately (don't block on MCP). Bridge is optional.
    let mut mail = mcp::MailPeek::empty(mcp::BridgeStatus::Offline);
    let skill_peek = skills::SkillPeek::from_builtin();
    if let Some(resp) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(fb_info) = resp.framebuffers().next() {
            // Always log geometry so UTM/QEMU serial shows why the window may be blank.
            {
                let mut msg = [0u8; 96];
                let s = b"fb: ";
                let mut n = 0;
                for &b in s {
                    msg[n] = b;
                    n += 1;
                }
                // tiny decimal helpers
                fn push_u32(buf: &mut [u8], n: &mut usize, mut v: u32) {
                    let mut tmp = [0u8; 10];
                    let mut i = 0;
                    if v == 0 {
                        tmp[0] = b'0';
                        i = 1;
                    } else {
                        while v > 0 {
                            tmp[i] = b'0' + (v % 10) as u8;
                            v /= 10;
                            i += 1;
                        }
                    }
                    while i > 0 {
                        i -= 1;
                        if *n < buf.len() {
                            buf[*n] = tmp[i];
                            *n += 1;
                        }
                    }
                }
                push_u32(&mut msg, &mut n, fb_info.width() as u32);
                msg[n] = b'x';
                n += 1;
                push_u32(&mut msg, &mut n, fb_info.height() as u32);
                msg[n] = b' ';
                n += 1;
                push_u32(&mut msg, &mut n, fb_info.bpp() as u32);
                msg[n] = b'b';
                n += 1;
                msg[n] = b'p';
                n += 1;
                msg[n] = b'p';
                n += 1;
                msg[n] = b'\n';
                n += 1;
                serial_port.write_bytes(&msg[..n]);
            }
            if let Some(screen) = unsafe {
                fb::Screen::new(
                    fb_info.addr(),
                    fb_info.width(),
                    fb_info.height(),
                    fb_info.pitch(),
                    fb_info.bpp(),
                    (
                        fb_info.red_mask_shift(),
                        fb_info.green_mask_shift(),
                        fb_info.blue_mask_shift(),
                    ),
                )
            } {
                // Everything composes in cached RAM; `present()` is the only
                // thing that touches video memory.
                let surface = screen.surface();
                ui::draw_home(surface, &mail, &skill_peek, "");
                screen.present();

                // Liveness only until the user consents. Reading the inbox
                // here would fetch — and, because the bridge indexes results,
                // persist to disk — mail before anyone agreed to it.
                let mut grants = caps::Caps::none();
                mail = mcp::MailPeek::empty(mcp::probe_bridge());
                match mail.status {
                    mcp::BridgeStatus::Online => serial_port.write_str("mcp: email connected\n"),
                    mcp::BridgeStatus::Offline => serial_port.write_str("mcp: email offline\n"),
                }
                serial_port.write_str("skills: builtins ready\n");
                ui::draw_home(surface, &mail, &skill_peek, "");

                let cx = surface.width() as i32 / 2;
                let cy = surface.height() as i32 / 2;
                mouse::paint_pointer(surface, cx, cy);
                screen.present();
                serial_port.write_str("mouse: pointer painted\n");

                serial::request_qemu_exit(true);

                let hhdm = HHDM_REQUEST
                    .get_response()
                    .map(|r| r.offset())
                    .unwrap_or(0);
                serial_port.write_str("mouse: probing usb\n");
                let mut tablet = None;
                let mut why = [0u8; 24];
                if let Some(mmap) = MEMORY_MAP_REQUEST.get_response() {
                    if let Some((p0, p1)) = usb_tablet::alloc_dma_pages(mmap) {
                        tablet = unsafe { usb_tablet::UsbTablet::init(hhdm, p0, p1, &mut why) };
                    } else {
                        let m = b"no-dma";
                        why[..m.len()].copy_from_slice(m);
                    }
                } else {
                    let m = b"no-mmap";
                    why[..m.len()].copy_from_slice(m);
                }
                if tablet.is_some() {
                    serial_port.write_str("mouse: usb-tablet ready\n");
                } else {
                    serial_port.write_str("mouse: usb-tablet missing ");
                    let n = why.iter().position(|&b| b == 0).unwrap_or(why.len());
                    serial_port.write_bytes(&why[..n]);
                    serial_port.write_str("\n");
                }

                let mut mice = mouse::Mouse::new(surface.width() as i32, surface.height() as i32);
                if mice.init() {
                    serial_port.write_str("mouse: ps2 ready\n");
                } else {
                    serial_port.write_str("mouse: ps2 init soft-fail\n");
                    mice.present = true;
                }

                ui::draw_home(surface, &mail, &skill_peek, "");
                let mut cursor = mouse::Cursor::new();
                let mut x = cx;
                let mut y = cy;
                let mut prev_buttons = 0u8;
                let mut status_buf = [0u8; 72];
                write_status(&mut status_buf, grants.footer_status());
                let mut setup = setup::Setup::new();
                let mut kb = keyboard::Keyboard::new();
                let mut query = keyboard::TextField::<{ searchui::QUERY_MAX }>::new();
                let mut sview = searchui::SearchView::new();
                let mut page = mcp::DocPage::empty(mcp::BridgeStatus::Offline, false);
                let mut open_title = keyboard::TextField::<{ searchui::QUERY_MAX }>::new();
                let mut view = screens::View::Home;
                let caret = true;
                // First boot: run the setup journey before the home screen.
                cursor.hide(surface);
                setup.draw(surface, &mail, &skill_peek);
                cursor.show_at(surface, x, y);
                enter(&screen);
                serial_port.write_str("ui: setup welcome\n");
                // Chime after the first frame is up, so the screen is never
                // waiting on the speaker.
                beep::startup();

                // Measure what a frame actually costs, rather than guessing.
                {
                    let t0 = serial::rdtsc();
                    screen.present_all();
                    let t1 = serial::rdtsc();
                    surface.mark_dirty(0, 0, 24, 32);
                    screen.present();
                    let t2 = serial::rdtsc();
                    serial_port.write_str("perf: full=");
                    write_u64(&serial_port, (t1 - t0) / 1000);
                    serial_port.write_str("kcyc dirty=");
                    write_u64(&serial_port, (t2 - t1) / 1000);
                    serial_port.write_str("kcyc\n");
                }

                loop {
                    let w = surface.width() as i32;
                    let h = surface.height() as i32;
                    let mut buttons = prev_buttons;
                    let mut moved = false;
                    if let Some(ref mut t) = tablet {
                        if t.poll(w, h) {
                            x = t.x;
                            y = t.y;
                            buttons = t.buttons;
                            mice.x = x;
                            mice.y = y;
                            mice.buttons = buttons;
                            moved = true;
                        }
                    } else if mice.poll(w, h) {
                        x = mice.x;
                        y = mice.y;
                        buttons = mice.buttons;
                        moved = true;
                    }

                    if !setup.is_finished() {
                        let before = setup.step;
                        if setup.pointer(x, y, buttons) {
                            // Entering the Bridge step: re-probe COM2 so the
                            // status card reflects a bridge that came up after boot.
                            if setup.step == setup::Step::Bridge && before != setup::Step::Bridge {
                                // Still pre-consent: the Capabilities step
                                // comes after this one, so probe, don't read.
                                mail = mcp::MailPeek::empty(mcp::probe_bridge());
                                serial_port.write_str(match mail.status {
                                    mcp::BridgeStatus::Online => "mcp: bridge live\n",
                                    mcp::BridgeStatus::Offline => "mcp: bridge still offline\n",
                                });
                            }
                            cursor.hide(surface);
                            if setup.is_finished() {
                                grants = setup.grants();
                                write_status(&mut status_buf, grants.footer_status());
                                serial_port.write_str("ui: setup done\n");
                                serial_port.write_str("caps: ");
                                serial_port.write_str(status_str(&status_buf));
                                serial_port.write_str("\n");
                                mail = mcp::fetch_mail_peek(grants);
                                ui::draw_home(
                                    surface,
                                    &mail,
                                    &skill_peek,
                                    status_str(&status_buf),
                                );
                            } else {
                                setup.draw(surface, &mail, &skill_peek);
                            }
                            cursor.show_at(surface, x, y);
                            enter(&screen);
                            moved = false;
                        }
                    } else if view == screens::View::Home {
                        // Type straight into the home field - no click first.
                        let mut dirty = false;
                        while let Some(key) = kb.poll() {
                            match key {
                                keyboard::Key::Enter => {
                                    if !query.is_empty() {
                                        sview.run_via(query.as_str(), grants);
                                        view = screens::View::Search;
                                        serial_port.write_str("search: ran from home\n");
                                        dirty = true;
                                    }
                                }
                                keyboard::Key::Escape => {
                                    if !query.is_empty() {
                                        query.clear();
                                        dirty = true;
                                    }
                                }
                                other => {
                                    if query.apply(other) {
                                        dirty = true;
                                    }
                                }
                            }
                        }
                        if dirty {
                            cursor.hide(surface);
                            if view == screens::View::Search {
                                searchui::draw(
                                    surface,
                                    &sview,
                                    query.as_str(),
                                    caret,
                                    bridge_note(&mail),
                                );
                            } else {
                                ui::draw_home_full(
                                    surface,
                                    &mail,
                                    &skill_peek,
                                    status_str(&status_buf),
                                    query.as_str(),
                                    caret,
                                );
                            }
                            cursor.show_at(surface, x, y);
                            enter(&screen);
                            moved = false;
                        }
                    }
                    if view != screens::View::Home {
                        // --- search screen: keyboard drives it ---
                        let mut dirty = false;
                        while let Some(key) = kb.poll() {
                            match key {
                                keyboard::Key::Enter => {
                                    if view == screens::View::Search {
                                        sview.run_via(query.as_str(), grants);
                                        serial_port.write_str("search: ran\n");
                                        dirty = true;
                                    }
                                }
                                keyboard::Key::Escape => {
                                    view = screens::View::Home;
                                    dirty = true;
                                }
                                other => {
                                    // Only the search screen has a field.
                                    // Without this, typing on Skills or
                                    // Capabilities silently built a query you
                                    // could not see.
                                    if view == screens::View::Search && query.apply(other) {
                                        dirty = true;
                                    }
                                }
                            }
                        }
                        // Clicking Back leaves the search screen.
                        let left_down = buttons & 0x01 != 0;
                        let was_down = prev_buttons & 0x01 != 0;
                        if left_down && !was_down {
                            let (bx, by, bw, bh) = searchui::back_rect(w);
                            if x >= bx && x < bx + bw && y >= by && y < by + bh {
                                // Back from the reader returns to results.
                                view = if view == screens::View::Reader {
                                    screens::View::Search
                                } else {
                                    screens::View::Home
                                };
                                dirty = true;
                            } else if view == screens::View::Search {
                                // Open a result.
                                if let Some(i) =
                                    searchui::result_hit(w, h, sview.count, x, y)
                                {
                                    let row = &sview.rows[i];
                                    open_title.clear();
                                    for b in row.title().bytes() {
                                        open_title.apply(keyboard::Key::Char(b));
                                    }
                                    page = mcp::fetch_doc(grants, row.url());
                                    view = screens::View::Reader;
                                    serial_port.write_str("ui: open doc\n");
                                    dirty = true;
                                }
                            } else if view == screens::View::Caps {
                                // Live switches: revoke or grant after setup.
                                if let Some(i) = screens::caps_hit(w, x, y) {
                                    let before = grants;
                                    grants = screens::toggle(grants, i);
                                    // Revoked? Have the host delete what that
                                    // grant produced.
                                    for (cap, tool) in [
                                        (caps::Cap::WorkspaceIndex, "workspace.forget"),
                                        (caps::Cap::AudioTranscribe, "audio.forget"),
                                    ] {
                                        if before.allows(cap) && !grants.allows(cap) {
                                            mcp::forget(tool);
                                            serial_port.write_str("caps: revoked ");
                                            serial_port.write_str(cap.name());
                                            serial_port.write_str(" - purged\n");
                                        }
                                    }
                                    write_status(&mut status_buf, grants.footer_status());
                                    dirty = true;
                                }
                            }
                        }
                        if dirty {
                            cursor.hide(surface);
                            match view {
                                screens::View::Search => searchui::draw(
                                    surface,
                                    &sview,
                                    query.as_str(),
                                    caret,
                                    bridge_note(&mail),
                                ),
                                screens::View::Skills => screens::draw_skills(surface, &skill_peek),
                                screens::View::Caps => screens::draw_caps(surface, grants),
                                screens::View::Reader => {
                                    searchui::draw_reader(surface, open_title.as_str(), &page)
                                }
                                screens::View::Home => {
                                    ui::draw_home(
                                        surface,
                                        &mail,
                                        &skill_peek,
                                        status_str(&status_buf),
                                    )
                                }
                            }
                            cursor.show_at(surface, x, y);
                            enter(&screen);
                            moved = false;
                        }
                    } else {
                        let left_down = buttons & 1 != 0;
                        let left_was = prev_buttons & 1 != 0;
                        if left_down && !left_was {
                            let targets = ui::home_targets(w, h, &skill_peek);
                            let mut clicked = false;
                            match targets.hit(x, y) {
                                Some(ui::HomeHit::Cta(ui::CtaId::Ready)) => {
                                    serial_port.write_str("ui: click Ready\n");
                                    setup = setup::Setup::new();
                                    cursor.hide(surface);
                                    setup.draw(surface, &mail, &skill_peek);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen);
                                    clicked = true;
                                    moved = false;
                                }
                                Some(ui::HomeHit::Cta(ui::CtaId::Skills))
                                | Some(ui::HomeHit::Card(ui::CardId::Skills)) => {
                                    serial_port.write_str("ui: click Skills\n");
                                    if grants.allows(caps::Cap::SkillsSave) {
                                        write_status(
                                            &mut status_buf,
                                            "skills.save granted - playbooks writable",
                                        );
                                    } else {
                                        write_status(
                                            &mut status_buf,
                                            "skills.save denied - playbooks read-only",
                                        );
                                    }
                                    clicked = true;
                                }
                                Some(ui::HomeHit::Card(ui::CardId::Connectors)) => {
                                    serial_port.write_str("ui: open search\n");
                                    view = screens::View::Search;
                                    query.clear();
                                    sview = searchui::SearchView::new();
                                    cursor.hide(surface);
                                    searchui::draw(
                                        surface,
                                        &sview,
                                        query.as_str(),
                                        caret,
                                        bridge_note(&mail),
                                    );
                                    cursor.show_at(surface, x, y);
                                    enter(&screen);
                                    clicked = true;
                                    moved = false;
                                }
                                #[allow(unreachable_patterns)]
                                Some(ui::HomeHit::Card(ui::CardId::Connectors)) => {
                                    serial_port.write_str("ui: click Connectors\n");
                                    mail = mcp::fetch_mail_peek(grants);
                                    let search = mcp::fetch_search_peek(grants, "capability");
                                    if search.denied {
                                        write_status(
                                            &mut status_buf,
                                            "search.query denied by caps",
                                        );
                                        serial_port.write_str("search: denied\n");
                                    } else if search.status == mcp::BridgeStatus::Offline {
                                        write_status(&mut status_buf, "bridge offline - no search");
                                        serial_port.write_str("search: offline\n");
                                    } else if search.count == 0 {
                                        write_status(&mut status_buf, "search: no hits");
                                        serial_port.write_str("search: n=0\n");
                                    } else {
                                        // "search: <title>" into the footer buffer.
                                        let title = search.title_at(0);
                                        let mut msg = [0u8; 72];
                                        let prefix = b"search: ";
                                        msg[..prefix.len()].copy_from_slice(prefix);
                                        let tn = title.len().min(72 - prefix.len() - 1);
                                        msg[prefix.len()..prefix.len() + tn]
                                            .copy_from_slice(&title.as_bytes()[..tn]);
                                        let n = prefix.len() + tn;
                                        write_status(
                                            &mut status_buf,
                                            core::str::from_utf8(&msg[..n]).unwrap_or("search: ok"),
                                        );
                                        serial_port.write_str("search: n=");
                                        let d = b'0' + (search.count.min(9) as u8);
                                        serial_port.write_bytes(&[d, b'\n']);
                                    }
                                    clicked = true;
                                }
                                Some(ui::HomeHit::Card(ui::CardId::Capabilities)) => {
                                    serial_port.write_str("ui: click Capabilities\n");
                                    write_status(&mut status_buf, grants.footer_status());
                                    clicked = true;
                                }
                                None => {}
                            }
                            if clicked && setup.is_finished() {
                                cursor.hide(surface);
                                ui::draw_home(
                                    surface,
                                    &mail,
                                    &skill_peek,
                                    status_str(&status_buf),
                                );
                                cursor.show_at(surface, x, y);
                                enter(&screen);
                                moved = false;
                            }
                        }
                    }
                    prev_buttons = buttons;
                    if moved {
                        cursor.show_at(surface, x, y);
                        // hide()/show_at() marked both footprints; present()
                        // blits exactly that union and nothing else.
                        screen.present();
                    }
                    core::hint::spin_loop();
                }
            } else {
                serial_port.write_str("fb: unsupported format\n");
            }
        } else {
            serial_port.write_str("fb: no framebuffer\n");
        }
    } else {
        serial_port.write_str("fb: limine framebuffer missing\n");
    }

    // Only the framebuffer-missing/unsupported paths reach here — that is a
    // boot failure, and the smoke test must see it as one.
    serial::exit_qemu(false);
}

/// One line telling the user where answers come from right now.
fn bridge_note(mail: &mcp::MailPeek) -> &'static str {
    match mail.status {
        mcp::BridgeStatus::Online => "Answers come from the local index and the host bridge.",
        mcp::BridgeStatus::Offline => "Bridge offline - answering from the index baked into the kernel.",
    }
}

/// Play a screen entrance: the frame is already composed in the back buffer.
///
/// Snapping between screens is what made this feel unlike a desktop; an
/// eased slide-and-fade costs a handful of blits and reads as intentional.
fn enter(screen: &fb::Screen) {
    let mut mark = serial::rdtsc();
    for i in 0..=anim::SLIDE_IN.frames {
        let (dy, a) = anim::SLIDE_IN.at(i);
        screen.present_slide(dy, a, ui::theme::BG);
        mark = anim::pace(mark, anim::SLIDE_IN.frame_us);
    }
}

/// Decimal u64 to COM1, for the perf line.
fn write_u64(port: &serial::Serial, mut v: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if v == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    port.write_bytes(&buf[i..]);
}

fn write_status(buf: &mut [u8; 72], s: &str) {
    buf.fill(0);
    let bytes = s.as_bytes();
    let n = bytes.len().min(buf.len().saturating_sub(1));
    buf[..n].copy_from_slice(&bytes[..n]);
}

fn status_str(buf: &[u8; 72]) -> &str {
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    core::str::from_utf8(&buf[..n]).unwrap_or("")
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    serial::exit_qemu(false);
}
