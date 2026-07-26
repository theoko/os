#![no_std]
#![no_main]

use core::hint::black_box;

use kernel::{anim, beep, caps, fb, keyboard, mcp, mouse, screens, searchui, serial, setup, skills, ui, usb_tablet, HELLO_MESSAGE};
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
    serial_port.write_str(HELLO_MESSAGE);
    serial_port.write_str("\n");

    // Liveness only until the user consents. Reading the inbox here would
    // fetch — and persist — mail before anyone agreed to it.
    let mut mail = mcp::MailPeek::empty(mcp::probe_bridge());
    match mail.status {
        mcp::BridgeStatus::Online => serial_port.write_str("mcp: email connected\n"),
        mcp::BridgeStatus::Offline => serial_port.write_str("mcp: email offline\n"),
    }
    serial_port.write_str("skills: builtins ready\n");
    let mut skill_peek = skills::SkillPeek::from_builtin();
    if let Some(resp) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(fb_info) = resp.framebuffers().next() {
            // Always log geometry so UTM/QEMU serial shows why the window may be blank.
            serial_port.write_str("fb: ");
            serial_port.write_u64(fb_info.width());
            serial_port.write_str("x");
            serial_port.write_u64(fb_info.height());
            serial_port.write_str(" ");
            serial_port.write_u64(fb_info.bpp() as u64);
            serial_port.write_str("bpp\n");
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
                let mut grants = caps::Caps::none();

                let cx = surface.width() as i32 / 2;
                let cy = surface.height() as i32 / 2;
                ui::draw_home_full(surface, &mail, &skill_peek, "", "", false);
                mouse::draw_arrow(surface, cx, cy);
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

                let mut cursor = mouse::Cursor::new();
                let mut x = cx;
                let mut y = cy;
                let mut prev_buttons = 0u8;
                let mut status_buf = [0u8; 72];
                skills::copy_field(&mut status_buf, grants.footer_status());
                let mut setup = setup::Setup::new();
                let mut kb = keyboard::Keyboard::new();
                let mut query = keyboard::TextField::<{ searchui::QUERY_MAX }>::new();
                let mut sview = searchui::SearchView::new();
                let mut page = mcp::DocPage::empty(mcp::BridgeStatus::Offline, false);
                let mut open_title = [0u8; searchui::QUERY_MAX];
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
                    serial_port.write_u64((t1 - t0) / 1000);
                    serial_port.write_str("kcyc dirty=");
                    serial_port.write_u64((t2 - t1) / 1000);
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
                                log_skill_source(&serial_port, &skill_peek);
                            }
                            if setup.is_finished() {
                                grants = setup.grants();
                                skills::copy_field(&mut status_buf, grants.footer_status());
                                serial_port.write_str("ui: setup done\n");
                                serial_port.write_str("caps: ");
                                serial_port.write_str(skills::str_at(&status_buf));
                                serial_port.write_str("\n");
                                mail = mcp::fetch_mail_peek(grants);
                                view = screens::View::Home;
                                repaint(
                                    &mut cursor,
                                    surface,
                                    &screen,
                                    x,
                                    y,
                                    &mut moved,
                                    view,
                                    &mail,
                                    &skill_peek,
                                    skills::str_at(&status_buf),
                                    "",
                                    false,
                                    &sview,
                                    "",
                                    &page,
                                    grants,
                                );
                            } else {
                                cursor.hide(surface);
                                setup.draw(surface, &mail, &skill_peek);
                                cursor.show_at(surface, x, y);
                                enter(&screen);
                                moved = false;
                            }
                        }
                    } else {
                        let mut dirty = false;
                        // Keep Home vs non-Home arms separate so a same-frame
                        // key that changes `view` cannot also run the other
                        // arm's click handler.
                        if view == screens::View::Home {
                            while let Some(key) = kb.poll() {
                                if handle_key(
                                    &mut view,
                                    key,
                                    &mut query,
                                    &mut sview,
                                    grants,
                                    &serial_port,
                                ) {
                                    dirty = true;
                                }
                            }
                            if click_edge(buttons, prev_buttons) {
                                let targets = ui::home_targets(w);
                                match targets.hit(x, y) {
                                    Some(ui::HomeHit::SearchField)
                                    | Some(ui::HomeHit::Card(ui::CardId::Search)) => {
                                        serial_port.write_str("ui: open search\n");
                                        view = screens::View::Search;
                                        query.clear();
                                        sview = searchui::SearchView::new();
                                        dirty = true;
                                    }
                                    Some(ui::HomeHit::Card(ui::CardId::Skills)) => {
                                        serial_port.write_str("ui: click Skills\n");
                                        skill_peek = mcp::fetch_skill_peek();
                                        log_skill_source(&serial_port, &skill_peek);
                                        view = screens::View::Skills;
                                        dirty = true;
                                    }
                                    Some(ui::HomeHit::Card(ui::CardId::Capabilities)) => {
                                        serial_port.write_str("ui: click Capabilities\n");
                                        skills::copy_field(&mut status_buf, grants.footer_status());
                                        view = screens::View::Caps;
                                        dirty = true;
                                    }
                                    None => {}
                                }
                            }
                        } else {
                            while let Some(key) = kb.poll() {
                                if handle_key(
                                    &mut view,
                                    key,
                                    &mut query,
                                    &mut sview,
                                    grants,
                                    &serial_port,
                                ) {
                                    dirty = true;
                                }
                            }
                            if click_edge(buttons, prev_buttons) {
                                if screens::back_rect().contains(x, y) {
                                    // Back from the reader returns to results.
                                    view = if view == screens::View::Reader {
                                        screens::View::Search
                                    } else {
                                        screens::View::Home
                                    };
                                    dirty = true;
                                } else if view == screens::View::Search {
                                    if let Some(i) =
                                        searchui::result_hit(w, sview.count, x, y)
                                    {
                                        let row = &sview.rows[i];
                                        skills::copy_field(&mut open_title, row.title());
                                        page = mcp::fetch_doc(grants, row.url());
                                        view = screens::View::Reader;
                                        serial_port.write_str("ui: open doc\n");
                                        dirty = true;
                                    }
                                } else if view == screens::View::Caps {
                                    if let Some(i) = screens::caps_hit(w, x, y) {
                                        let before = grants;
                                        grants = screens::toggle(grants, i);
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
                                        skills::copy_field(
                                            &mut status_buf,
                                            grants.footer_status(),
                                        );
                                        dirty = true;
                                    }
                                } else if view == screens::View::Skills {
                                    if let Some(i) =
                                        screens::skills_hit(w, skill_peek.count, x, y)
                                    {
                                        let name = skill_peek.name_at(i);
                                        let mut blurb = [0u8; 72];
                                        if mcp::fetch_skill_blurb(name, &mut blurb) {
                                            skills::copy_field(
                                                &mut status_buf,
                                                skills::str_at(&blurb),
                                            );
                                            serial_port.write_str("skills: got ");
                                            serial_port.write_str(name);
                                            serial_port.write_str("\n");
                                        } else {
                                            skills::copy_field(&mut status_buf, name);
                                            serial_port.write_str("skills: get offline ");
                                            serial_port.write_str(name);
                                            serial_port.write_str("\n");
                                        }
                                        dirty = true;
                                    }
                                }
                            }
                        }
                        if dirty {
                            repaint(
                                &mut cursor,
                                surface,
                                &screen,
                                x,
                                y,
                                &mut moved,
                                view,
                                &mail,
                                &skill_peek,
                                skills::str_at(&status_buf),
                                query.as_str(),
                                caret,
                                &sview,
                                skills::str_at(&open_title),
                                &page,
                                grants,
                            );
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

/// Rising edge on the primary mouse button.
fn click_edge(buttons: u8, prev: u8) -> bool {
    buttons & 1 != 0 && prev & 1 == 0
}

/// Keyboard for Home / Search (and Escape-to-home elsewhere).
///
/// Returns whether the frame needs a repaint.
fn handle_key(
    view: &mut screens::View,
    key: keyboard::Key,
    query: &mut keyboard::TextField<{ searchui::QUERY_MAX }>,
    sview: &mut searchui::SearchView,
    grants: caps::Caps,
    serial: &serial::Serial,
) -> bool {
    match (*view, key) {
        (screens::View::Home, keyboard::Key::Enter) => {
            if query.is_empty() {
                return false;
            }
            sview.run_via(query.as_str(), grants);
            *view = screens::View::Search;
            serial.write_str("search: ran from home\n");
            true
        }
        (screens::View::Home, keyboard::Key::Escape) => {
            if query.is_empty() {
                return false;
            }
            query.clear();
            true
        }
        (screens::View::Home, other) => query.apply(other),
        (screens::View::Search, keyboard::Key::Enter) => {
            sview.run_via(query.as_str(), grants);
            serial.write_str("search: ran\n");
            true
        }
        (screens::View::Search, other) => match other {
            keyboard::Key::Escape => {
                *view = screens::View::Home;
                true
            }
            k => query.apply(k),
        },
        // Skills / Caps / Reader: Escape returns home; typing is ignored.
        (_, keyboard::Key::Escape) => {
            *view = screens::View::Home;
            true
        }
        _ => false,
    }
}

/// One line telling the user where answers come from right now.
fn bridge_note(mail: &mcp::MailPeek) -> &'static str {
    match mail.status {
        mcp::BridgeStatus::Online => "Answers come from the local index and the host bridge.",
        mcp::BridgeStatus::Offline => mcp::BRIDGE_OFFLINE_HINT,
    }
}

fn log_skill_source(port: &serial::Serial, peek: &skills::SkillPeek) {
    port.write_str(if peek.from_bridge {
        "skills: listed from bridge\n"
    } else {
        "skills: builtins (bridge offline)\n"
    });
}

/// Hide cursor, paint the active view, show cursor, play entrance.
fn repaint(
    cursor: &mut mouse::Cursor,
    surface: &fb::Surface,
    screen: &fb::Screen,
    x: i32,
    y: i32,
    moved: &mut bool,
    view: screens::View,
    mail: &mcp::MailPeek,
    skills: &skills::SkillPeek,
    status: &str,
    query: &str,
    caret: bool,
    sview: &searchui::SearchView,
    open_title: &str,
    page: &mcp::DocPage,
    grants: caps::Caps,
) {
    cursor.hide(surface);
    match view {
        screens::View::Search => {
            searchui::draw(surface, sview, query, caret, bridge_note(mail))
        }
        screens::View::Skills => screens::draw_skills(surface, skills),
        screens::View::Caps => screens::draw_caps(surface, grants),
        screens::View::Reader => searchui::draw_reader(surface, open_title, page),
        screens::View::Home => {
            ui::draw_home_full(surface, mail, skills, status, query, caret)
        }
    }
    cursor.show_at(surface, x, y);
    enter(screen);
    *moved = false;
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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    serial::exit_qemu(false);
}
