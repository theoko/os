#![no_std]
#![no_main]

use core::hint::black_box;

use kernel::{anim, beep, caps, fb, keyboard, mcp, mouse, screens, searchui, serial, setup, skills, ui, usb_tablet};
use limine::BaseRevision;
use limine::request::{
    FramebufferRequest, HhdmRequest, MemoryMapRequest, RequestsEndMarker, RequestsStartMarker,
    StackSizeRequest,
};

/// Early-boot COM1 banner (must match `scripts/smoke_common.py` HELLO).
const HELLO: &str = "os: hello from kernel";
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
        serial::exit_qemu();
    }

    let serial_port = serial::Serial::com1();
    serial_port.init();
    serial_port.write_str(HELLO);
    serial_port.write_str("\n");

    // Caps::none(): PING only until the user consents (default_grants would
    // CALL email.search against the host mailbox).
    let mut mail = mcp::fetch_mail_peek(caps::Caps::none());
    log_bridge_status(&serial_port, mail.online());
    let mut skill_peek = skills::SkillPeek::from_builtins();
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

                let cx = surface.width() as i32 / 2;
                let cy = surface.height() as i32 / 2;
                let mut cursor = mouse::Cursor::new();
                // Blank + cursor for the QEMU smoke present; setup paints next.
                surface.fill();
                cursor.show_at(surface, cx, cy);
                screen.present();
                serial_port.write_str("mouse: pointer painted\n");

                serial::request_qemu_exit();

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
                }

                let mut setup = setup::Setup::new();
                let mut kb = keyboard::Keyboard::new();
                let mut query = keyboard::TextField::<{ searchui::QUERY_MAX }>::new();
                let mut sview = searchui::SearchView::new();
                let mut page = mcp::DocPage::empty(mcp::DocOutcome::Offline);
                let mut view = screens::View::Home;
                // First boot: run the setup journey before the home screen.
                cursor.hide(surface);
                setup.draw(surface, mail.online(), &skill_peek);
                cursor.show_at(surface, mice.x, mice.y);
                anim::slide_in(|dy, a| screen.present_slide(dy, a));
                serial_port.write_str("ui: setup welcome\n");
                // Chime after the first frame is up, so the screen is never
                // waiting on the speaker.
                beep::startup();

                loop {
                    let w = surface.width() as i32;
                    let mut moved = false;
                    if let Some(ref mut t) = tablet {
                        if t.poll(&mut mice) {
                            moved = true;
                        }
                    } else if mice.poll() {
                        moved = true;
                    }

                    let clicked = mice.take_click_edge();
                    if !setup.is_finished() {
                        let before = setup.step;
                        if clicked && setup.click(mice.x, mice.y) {
                            // Entering the Bridge step: re-probe COM2 so the
                            // status card reflects a bridge that came up after boot.
                            if setup.step == setup::Step::Bridge && before != setup::Step::Bridge {
                                // Still pre-consent: Caps::none() PINGs only
                                // (setup.caps already has default_grants).
                                mail = mcp::fetch_mail_peek(caps::Caps::none());
                                log_bridge_status(&serial_port, mail.online());
                            }
                            if setup.step == setup::Step::Skills && before != setup::Step::Skills {
                                skill_peek = mcp::fetch_skill_peek();
                            }
                            if setup.is_finished() {
                                serial_port.write_str("ui: setup done\n");
                                serial_port.write_str("caps: ");
                                serial_port.write_u64(setup.caps.granted_count() as u64);
                                serial_port.write_str(" granted\n");
                                mail = mcp::fetch_mail_peek(setup.caps);
                                view = screens::View::Home;
                                repaint(
                                    &mut cursor,
                                    surface,
                                    &screen,
                                    mice.x,
                                    mice.y,
                                    view,
                                    &mail,
                                    &skill_peek,
                                    "",
                                    &sview,
                                    &page,
                                    setup.caps,
                                );
                                moved = false;
                            } else {
                                cursor.hide(surface);
                                setup.draw(surface, mail.online(), &skill_peek);
                                cursor.show_at(surface, mice.x, mice.y);
                                anim::slide_in(|dy, a| screen.present_slide(dy, a));
                                moved = false;
                            }
                        }
                    } else {
                        let mut dirty = false;
                        // Snapshot Home before keys so a same-frame view change
                        // cannot also run the other arm's click handler.
                        let on_home = view == screens::View::Home;
                        while let Some(key) = kb.poll() {
                            if handle_key(&mut view, key, &mut query, &mut sview, setup.caps) {
                                dirty = true;
                            }
                        }
                        if clicked {
                            if on_home {
                                match ui::home_hit(w, mice.x, mice.y) {
                                    Some(ui::HomeHit::SearchField)
                                    | Some(ui::HomeHit::Card(ui::CardId::Search)) => {
                                        view = screens::View::Search;
                                        query.clear();
                                        sview = searchui::SearchView::new();
                                        dirty = true;
                                    }
                                    Some(ui::HomeHit::Connect) => {
                                        mail = mcp::fetch_mail_peek(setup.caps);
                                        log_bridge_status(&serial_port, mail.online());
                                        dirty = true;
                                    }
                                    Some(ui::HomeHit::Card(ui::CardId::Skills)) => {
                                        skill_peek = mcp::fetch_skill_peek();
                                        view = screens::View::Skills;
                                        dirty = true;
                                    }
                                    Some(ui::HomeHit::Card(ui::CardId::Capabilities)) => {
                                        view = screens::View::Caps;
                                        dirty = true;
                                    }
                                    None => {}
                                }
                            } else if screens::back_hit(mice.x, mice.y) {
                                // Back from the reader returns to results.
                                view = if view == screens::View::Reader {
                                    screens::View::Search
                                } else {
                                    screens::View::Home
                                };
                                dirty = true;
                            } else if view == screens::View::Search {
                                if let Some(i) = sview.hit(w, mice.x, mice.y) {
                                    let (title, url) = sview.at(i);
                                    page = mcp::fetch_doc(setup.caps, url, title);
                                    view = screens::View::Reader;
                                    dirty = true;
                                }
                            } else if view == screens::View::Caps {
                                if let Some(i) = screens::caps_hit(w, mice.x, mice.y) {
                                    let before = setup.caps;
                                    setup.caps.toggle(i);
                                    let cap = caps::Cap::ALL[i];
                                    if before.allows(cap)
                                        && !setup.caps.allows(cap)
                                        && mcp::forget(cap)
                                    {
                                        serial_port.write_str("caps: revoked ");
                                        serial_port.write_str(cap.name());
                                        serial_port.write_str(" - purged\n");
                                    }
                                    dirty = true;
                                }
                            }
                        }
                        if dirty {
                            repaint(
                                &mut cursor,
                                surface,
                                &screen,
                                mice.x,
                                mice.y,
                                view,
                                &mail,
                                &skill_peek,
                                query.as_str(),
                                &sview,
                                &page,
                                setup.caps,
                            );
                            moved = false;
                        }
                    }
                    if moved {
                        cursor.show_at(surface, mice.x, mice.y);
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
    serial::exit_qemu();
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
) -> bool {
    match (*view, key) {
        (screens::View::Home | screens::View::Search, keyboard::Key::Enter) => {
            if query.as_str().trim().is_empty() {
                return false;
            }
            sview.run_via(query.as_str(), grants);
            *view = screens::View::Search;
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
        (screens::View::Search, other) => match other {
            keyboard::Key::Escape => {
                *view = screens::View::Home;
                true
            }
            k => query.apply(k),
        },
        // Reader Escape matches Back (results); Skills / Caps Escape → Home.
        (screens::View::Reader, keyboard::Key::Escape) => {
            *view = screens::View::Search;
            true
        }
        (_, keyboard::Key::Escape) => {
            *view = screens::View::Home;
            true
        }
        _ => false,
    }
}

fn log_bridge_status(port: &serial::Serial, online: bool) {
    port.write_str(if online {
        "mcp: bridge live\n"
    } else {
        "mcp: bridge still offline\n"
    });
}

/// Hide cursor, paint the active view, show cursor, play entrance.
fn repaint(
    cursor: &mut mouse::Cursor,
    surface: &fb::Surface,
    screen: &fb::Screen,
    x: i32,
    y: i32,
    view: screens::View,
    mail: &mcp::MailPeek,
    skills: &skills::SkillPeek,
    query: &str,
    sview: &searchui::SearchView,
    page: &mcp::DocPage,
    grants: caps::Caps,
) {
    cursor.hide(surface);
    match view {
        screens::View::Search => {
            searchui::draw(surface, sview, query, mail.online())
        }
        screens::View::Skills => screens::draw_skills(surface, skills),
        screens::View::Caps => screens::draw_caps(surface, grants),
        screens::View::Reader => searchui::draw_reader(surface, page),
        screens::View::Home => ui::draw_home(surface, mail, skills, grants, query),
    }
    cursor.show_at(surface, x, y);
    // Frame is already composed; eased slide-and-fade reads as intentional.
    anim::slide_in(|dy, a| screen.present_slide(dy, a));
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    serial::exit_qemu();
}
