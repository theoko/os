#![no_std]
#![no_main]

use core::hint::black_box;

use kernel::{agent, anim, beep, caps, fb, hello_message, keyboard, mcp, mouse, screens, searchui, serial, setup, skills, ui, usb_tablet};
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
    let mut skill_peek = skills::SkillPeek::from_builtin();
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
                let boot_brief = agent::Brief::empty();
                ui::draw_home(surface, &mail, &skill_peek, "", caps::Caps::none(), &boot_brief);
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
                ui::draw_home(surface, &mail, &skill_peek, "", caps::Caps::none(), &boot_brief);

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

                let mut brief = agent::Brief::empty();
                ui::draw_home(surface, &mail, &skill_peek, "", grants, &brief);
                let mut cursor = mouse::Cursor::new();
                let mut x = cx;
                let mut y = cy;
                let mut prev_buttons = 0u8;
                let mut status_buf = [0u8; 72];
                let mut status_len = grants.describe(&mut status_buf);
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
                            if setup.step == setup::Step::Skills && before != setup::Step::Skills {
                                skill_peek = mcp::fetch_skill_peek();
                                serial_port.write_str(if skill_peek.from_bridge {
                                    "skills: listed from bridge\n"
                                } else {
                                    "skills: builtins (bridge offline)\n"
                                });
                            }
                            cursor.hide(surface);
                            if setup.is_finished() {
                                grants = setup.grants();
                                status_len = grants.describe(&mut status_buf);
                                serial_port.write_str("ui: setup done\n");
                                serial_port.write_str("caps: ");
                                serial_port.write_str(status_str(&status_buf, status_len));
                                serial_port.write_str("\n");
                                if grants.allows(caps::Cap::WorkspaceIndex) {
                                    // Chosen during setup: build it now rather
                                    // than leaving an empty index behind a
                                    // switch that reads as on.
                                    mcp::build_index("workspace.index");
                                    serial_port.write_str("caps: indexing workspace\n");
                                }
                                if grants.allows(caps::Cap::PortalSync) {
                                    // Teddy API (corpus) + a live portal warm so
                                    // Online services is not an empty promise.
                                    mcp::build_index("tsearch.sync");
                                    mcp::build_index("teddy.health");
                                    mcp::build_index("market.health");
                                    serial_port.write_str("caps: warming teddy + market portals\n");
                                }
                                mail = mcp::fetch_mail_peek(grants);
                                // First act: run the plan/act skill under the
                                // grants just chosen so home is never empty
                                // theatre — the OS does something immediately.
                                brief = agent::morning(grants);
                                view = screens::View::Brief;
                                serial_port.write_str("agent: morning brief\n");
                                screens::draw_brief(surface, &brief);
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
                                    status_str(&status_buf, status_len),
                                    query.as_str(),
                                    caret,
                                    grants,
                                    &brief,
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
                                    view = if view == screens::View::Brief {
                                        screens::View::Home
                                    } else if view == screens::View::Reader {
                                        screens::View::Search
                                    } else {
                                        screens::View::Home
                                    };
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
                                    for (cap, forget_tool, build_tools) in [
                                        (
                                            caps::Cap::WorkspaceIndex,
                                            "workspace.forget",
                                            &["workspace.index"][..],
                                        ),
                                        (caps::Cap::AudioTranscribe, "audio.forget", &[][..]),
                                        // Corpus sync + warm a live teddy portal
                                        // so the switch is never on with nothing
                                        // behind it.
                                        (
                                            caps::Cap::PortalSync,
                                            "portal.forget",
                                            &["tsearch.sync", "teddy.health", "market.health"][..],
                                        ),
                                    ] {
                                        let was = before.allows(cap);
                                        let now = grants.allows(cap);
                                        if was && !now && !forget_tool.is_empty() {
                                            mcp::forget(forget_tool);
                                            serial_port.write_str("caps: revoked ");
                                            serial_port.write_str(cap.name());
                                            serial_port.write_str(" - purged\n");
                                        } else if !was && now {
                                            for t in build_tools {
                                                mcp::build_index(t);
                                            }
                                            if !build_tools.is_empty() {
                                                serial_port.write_str("caps: granted ");
                                                serial_port.write_str(cap.name());
                                                serial_port.write_str(" - ready\n");
                                            }
                                        }
                                    }
                                    status_len = grants.describe(&mut status_buf);
                                    dirty = true;
                                }
                            } else if view == screens::View::Skills {
                                if screens::skills_save_hit(
                                    w,
                                    skill_peek.count,
                                    grants.allows(caps::Cap::SkillsSave),
                                    x,
                                    y,
                                ) {
                                    match mcp::save_skill(
                                        grants,
                                        "guest-starter",
                                        "Starter from the Skills screen",
                                    ) {
                                        mcp::SaveSkillStatus::Ok => {
                                            write_status(&mut status_buf, "Saved guest-starter");
                                            serial_port.write_str("skills: saved guest-starter\n");
                                            skill_peek = mcp::fetch_skill_peek();
                                        }
                                        mcp::SaveSkillStatus::Denied => {
                                            write_status(&mut status_buf, "Grant Save skills first");
                                            serial_port.write_str("skills: save need skills.save\n");
                                        }
                                        mcp::SaveSkillStatus::Offline => {
                                            write_status(&mut status_buf, "Bridge offline");
                                            serial_port.write_str("skills: save offline\n");
                                        }
                                        mcp::SaveSkillStatus::Failed => {
                                            write_status(&mut status_buf, "skills.save failed");
                                            serial_port.write_str("skills: save failed\n");
                                        }
                                    }
                                    dirty = true;
                                } else if let Some(i) =
                                    screens::skills_hit(w, skill_peek.count, x, y)
                                {
                                    let name = skill_peek.name_at(i);
                                    if agent::is_runnable(name) {
                                        brief = agent::run(name, grants);
                                        view = screens::View::Brief;
                                        serial_port.write_str("agent: run ");
                                        serial_port.write_str(name);
                                        serial_port.write_str("\n");
                                        if brief.denied {
                                            serial_port.write_str("agent: need ");
                                            serial_port.write_str(brief.deny_name());
                                            serial_port.write_str("\n");
                                        }
                                    } else {
                                        let mut blurb = [0u8; 72];
                                        if mcp::fetch_skill_blurb(name, &mut blurb) {
                                            let n = blurb
                                                .iter()
                                                .position(|&b| b == 0)
                                                .unwrap_or(blurb.len());
                                            write_status(
                                                &mut status_buf,
                                                core::str::from_utf8(&blurb[..n]).unwrap_or(name),
                                            );
                                            serial_port.write_str("skills: got ");
                                            serial_port.write_str(name);
                                            serial_port.write_str("\n");
                                        } else {
                                            write_status(&mut status_buf, name);
                                            serial_port.write_str("skills: get offline ");
                                            serial_port.write_str(name);
                                            serial_port.write_str("\n");
                                        }
                                    }
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
                                screens::View::Skills => {
                                    screens::draw_skills(surface, &skill_peek, grants)
                                }
                                screens::View::Caps => screens::draw_caps(surface, grants),
                                screens::View::Brief => screens::draw_brief(surface, &brief),
                                screens::View::Reader => {
                                    searchui::draw_reader(surface, open_title.as_str(), &page)
                                }
                                screens::View::Home => {
                                    ui::draw_home(
                                        surface,
                                        &mail,
                                        &skill_peek,
                                        status_str(&status_buf, status_len),
                                        grants,
                                        &brief,
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
                            let targets = ui::home_targets(w, h, &skill_peek, &brief);
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
                                    skill_peek = mcp::fetch_skill_peek();
                                    serial_port.write_str(if skill_peek.from_bridge {
                                        "skills: listed from bridge\n"
                                    } else {
                                        "skills: builtins (bridge offline)\n"
                                    });
                                    view = screens::View::Skills;
                                    cursor.hide(surface);
                                    screens::draw_skills(surface, &skill_peek, grants);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen);
                                    // Don't fall through to the home redraw below.
                                    clicked = false;
                                    moved = false;
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
                                Some(ui::HomeHit::Card(ui::CardId::Capabilities)) => {
                                    serial_port.write_str("ui: click Capabilities\n");
                                    status_len = grants.describe(&mut status_buf);
                                    view = screens::View::Caps;
                                    cursor.hide(surface);
                                    screens::draw_caps(surface, grants);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen);
                                    clicked = false;
                                    moved = false;
                                }
                                Some(ui::HomeHit::Brief) => {
                                    serial_port.write_str("ui: reopen brief\n");
                                    view = screens::View::Brief;
                                    cursor.hide(surface);
                                    screens::draw_brief(surface, &brief);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen);
                                    clicked = false;
                                    moved = false;
                                }
                                None => {}
                            }
                            if clicked && setup.is_finished() {
                                cursor.hide(surface);
                                ui::draw_home(
                                    surface,
                                    &mail,
                                    &skill_peek,
                                    status_str(&status_buf, status_len),
                                    grants,
                                    &brief,
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

fn status_str(buf: &[u8], len: usize) -> &str {
    core::str::from_utf8(&buf[..len.min(buf.len())]).unwrap_or("")
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    serial::exit_qemu(false);
}
