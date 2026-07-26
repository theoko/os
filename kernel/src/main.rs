#![no_std]
#![no_main]

use core::hint::black_box;

use kernel::{
    agent, anim, beep, caps, fault, fb, hello_message, inputdiag, keyboard, level, mcp, mouse, pci,
    screens, searchui, serial, setup, skills, ui, usb_tablet,
};
use limine::BaseRevision;
use limine::request::{
    FramebufferRequest, HhdmRequest, MemoryMapRequest, RequestsEndMarker, RequestsStartMarker,
    StackSizeRequest,
};

const STACK_SIZE: u64 = 128 * 1024;
/// Chill game loop: always paced at 60 Hz so ambient motion keeps breathing
/// even when the pointer is still.
const FRAME_US: u32 = anim::FRAME_US_60;

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

    // Before the first byte: on aarch64 the MMU is already on and the kernel
    // runs in the higher half, so the PL011's physical address is not a
    // pointer. Locating it via the HHDM has to happen before any output,
    // because getting it wrong is a data abort into a vector table that does
    // not exist yet — the machine simply stops, saying nothing.
    // NOTE: intentionally NOT calling serial::locate_pl011 yet — Limine's HHDM
    // maps RAM, not device MMIO, so the PL011 is unreachable until the kernel
    // maps it itself. Until then the early console stays disabled rather than
    // aborting on first write. See serial::locate_pl011.

    let serial_port = serial::Serial::com1();
    serial_port.init();

    // Before anything can fault. Without this a bad pointer or an overflow
    // check triple-faults and the machine silently resets, which is
    // indistinguishable from "it just randomly crashes".
    fault::init();

    serial_port.write_str(hello_message());
    serial_port.write_str(serial::LINE_ENDING);

    // Paint UI immediately (don't block on MCP). Bridge is optional.
    let mut mail = mcp::MailPeek::empty(mcp::BridgeStatus::Offline);
    let mut files = mcp::FilePeek::empty(mcp::BridgeStatus::Offline, false);
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
                ui::draw_home(
                    surface,
                    &mail,
                    &files,
                    &skill_peek,
                    "",
                    caps::Caps::none(),
                    &boot_brief,
                    level::Level::Guided,
                );
                screen.present();

                // Liveness only until the user consents. Reading the inbox
                // here would fetch — and, because the bridge indexes results,
                // persist to disk — mail before anyone agreed to it.
                let mut grants = caps::Caps::none();
                let mut level = level::Level::Guided;
                mail = mcp::MailPeek::empty(mcp::probe_bridge());
                match mail.status {
                    mcp::BridgeStatus::Online => serial_port.write_str("mcp: email connected\n"),
                    mcp::BridgeStatus::Offline => serial_port.write_str("mcp: email offline\n"),
                }
                serial_port.write_str("skills: builtins ready\n");
                ui::draw_home(
                    surface,
                    &mail,
                    &files,
                    &skill_peek,
                    "",
                    caps::Caps::none(),
                    &boot_brief,
                    level,
                );

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
                    #[cfg(target_arch = "x86_64")]
                    {
                        mice.present = true;
                    }
                }

                let mut brief = agent::Brief::empty();
                ui::draw_home(surface, &mail, &files, &skill_peek, "", grants, &brief, level);
                let mut cursor = mouse::Cursor::new();
                let mut x = cx;
                let mut y = cy;
                let mut motion = mouse::CursorMotion::new(x, y);
                let mut frame_mark = serial::rdtsc();
                let mut prev_buttons = 0u8;
                #[cfg(target_arch = "aarch64")]
                let mut arm_cursor_refresh = 0u16;
                // What actually came up. Under QEMU this is always fine; on
                // real hardware it is the whole story, and a machine with no
                // driveable pointer renders a perfect home screen with a
                // cursor that never moves - indistinguishable from a hang.
                let inputs = inputdiag::Inputs {
                    ps2_controller: keyboard::Keyboard::present(),
                    ps2_keyboard: keyboard::Keyboard::present(),
                    ps2_mouse: mice.present,
                    usb_tablet: tablet.is_some(),
                    usb: pci::usb_survey(),
                };
                if let Some(note) = inputs.note() {
                    serial_port.write_str("input: ");
                    serial_port.write_str(note);
                    serial_port.write_str("\n");
                }

                let mut status_buf = [0u8; 128];
                // A dead pointer outranks the capability summary: it is the
                // only thing the person can act on.
                let mut status_len = match inputs.note() {
                    Some(note) => {
                        let n = note.len().min(status_buf.len());
                        status_buf[..n].copy_from_slice(&note.as_bytes()[..n]);
                        n
                    }
                    None => grants.describe(&mut status_buf),
                };

                // Paint it now, not on the next redraw.
                //
                // Home was already drawn above, and every later redraw is
                // triggered by input. On a machine with no input driver that
                // redraw never comes, so the one message explaining why
                // nothing responds was only ever shown to people whose input
                // already worked. The arm64 guest sat there displaying the
                // capability summary instead.
                if inputs.note().is_some() {
                    ui::draw_home(
                        surface,
                        &mail,
                        &files,
                        &skill_peek,
                        status_str(&status_buf, status_len),
                        grants,
                        &brief,
                        level,
                    );
                    screen.present_all();
                }

                let mut setup = setup::Setup::new();
                let animate = can_animate(&screen);
                serial_port.write_str(if animate {
                    "ui: transitions on\n"
                } else {
                    "ui: transitions off (full blit too slow)\n"
                });
                let mut kb = keyboard::Keyboard::new();
                let mut query = keyboard::TextField::<{ searchui::QUERY_MAX }>::new();
                let mut sview = searchui::SearchView::new();
                let mut page = mcp::DocPage::empty(mcp::BridgeStatus::Offline, false);
                let mut portal =
                    mcp::PortalStatus { reachable: false, cached: false, syncing: false, docs: 0 };
                let mut scroll = 0usize;
                let mut open_title = keyboard::TextField::<{ searchui::QUERY_MAX }>::new();
                let mut playbook = skills::workflow_for("agent-plan-act");
                let mut playbook_step = 0usize;
                let mut playbook_goal = keyboard::TextField::<{ searchui::QUERY_MAX }>::new();
                // Which catalog skill the open playbook belongs to, so the
                // approved final step can run that skill rather than a guess.
                let mut playbook_name = [0u8; 28];
                let mut playbook_name_len = 0usize;
                let mut view = screens::View::Home;
                let mut tick: u32 = 0;
                let mut caret = true;
                // First boot: run the setup journey before the home screen.
                cursor.hide(surface);
                setup.draw(surface, &mail, &skill_peek);
                cursor.show_at(surface, x, y);
                enter(&screen, animate, &mut motion, x, y);
                // An ARM guest can have a framebuffer before it has a native
                // pointer device. Keep the software cursor on the final frame
                // so the screen never looks frozen while that driver is absent.
                #[cfg(target_arch = "aarch64")]
                {
                    // Do not use the save/restore cursor here: QemuRamFB may
                    // repaint after the transition and restore its saved page
                    // over the arrow. A direct paint is persistent at rest.
                    cursor.hide(surface);
                    mouse::paint_pointer(surface, x, y);
                }
                #[cfg(not(target_arch = "aarch64"))]
                cursor.show_at(surface, x, y);
                screen.present();
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
                    if moved {
                        motion.set_target(x, y);
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
                                level = setup.level;
                                status_len = grants.describe(&mut status_buf);
                                serial_port.write_str("ui: setup done\n");
                                serial_port.write_str("ui: level ");
                                serial_port.write_str(level.serial_tag());
                                serial_port.write_str("\n");
                                serial_port.write_str("caps: ");
                                serial_port.write_str(status_str(&status_buf, status_len));
                                serial_port.write_str("\n");
                                // Anything granted during setup has to be built
                                // now. Only workspace was handled here, so
                                // enabling Online services at setup left the
                                // switch on with nothing behind it — the
                                // sync only fired if you toggled it later.
                                for (cap, tool, note) in [
                                    (
                                        caps::Cap::WorkspaceIndex,
                                        "workspace.index",
                                        "caps: indexing workspace\n",
                                    ),
                                    (
                                        caps::Cap::PortalSync,
                                        "tsearch.sync",
                                        "caps: syncing teddy\n",
                                    ),
                                ] {
                                    if grants.allows(cap) {
                                        serial_port.write_str(note);
                                        mcp::build_index(tool);
                                    }
                                }
                                if grants.allows(caps::Cap::AudioTranscribe) {
                                    // No host path at setup — Search is the
                                    // picker (type /path/to.wav, Enter).
                                    serial_port.write_str(
                                        "caps: recordings ready - type a media path in Search\n",
                                    );
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
                                files = mcp::fetch_files_peek(grants);
                                // First act: run the plan/act skill under the
                                // grants just chosen so home is never empty
                                // theatre — the OS does something immediately.
                                brief = agent::morning(grants, level);
                                view = screens::View::Brief;
                                serial_port.write_str("agent: morning brief\n");
                                screens::draw_brief(surface, &brief);
                            } else {
                                setup.draw(surface, &mail, &skill_peek);
                            }
                            cursor.show_at(surface, x, y);
                            enter(&screen, animate, &mut motion, x, y);
                            moved = false;
                        }
                    } else if view == screens::View::Home {
                        // The nav dot is the only nav affordance; clicking it
                        // shows every source's state, which the dot alone
                        // cannot express.
                        let left_down = buttons & 0x01 != 0;
                        let was_down = prev_buttons & 0x01 != 0;
                        if left_down && !was_down && ui::status_dot_rect(w).contains(x, y) {
                            portal = mcp::portal_status();
                            view = screens::View::Status;
                            cursor.hide(surface);
                            screens::draw_status(surface, &mail, &portal, grants);
                            cursor.show_at(surface, x, y);
                            enter(&screen, animate, &mut motion, x, y);
                            moved = false;
                        }

                        // Type straight into the home field - no click first.
                        let mut dirty = false;
                        while let Some(key) = kb.poll() {
                            match key {
                                keyboard::Key::Enter => {
                                    if !query.is_empty() {
                                        if try_transcribe_path(
                                            &mut sview,
                                            &serial_port,
                                            &mut status_buf,
                                            grants,
                                            query.as_str(),
                                        ) {
                                            view = screens::View::Search;
                                            dirty = true;
                                        } else {
                                            // Agentic home: plan/act under caps,
                                            // then a Brief with openable Doc rows.
                                            brief = agent::run_goal(query.as_str(), grants);
                                            view = screens::View::Brief;
                                            serial_port.write_str("agent: ran goal\n");
                                            dirty = true;
                                        }
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
                            match view {
                                screens::View::Search => searchui::draw(
                                    surface,
                                    &sview,
                                    query.as_str(),
                                    caret,
                                    bridge_note(&mail),
                                    level,
                                ),
                                screens::View::Brief => screens::draw_brief(surface, &brief),
                                screens::View::Reader => {
                                    searchui::draw_reader(
                                        surface,
                                        open_title.as_str(),
                                        &page,
                                        scroll,
                                    )
                                }
                                _ => ui::draw_home_full(
                                    surface,
                                    &mail,
                                    &files,
                                    &skill_peek,
                                    status_str(&status_buf, status_len),
                                    query.as_str(),
                                    caret,
                                    grants,
                                    &brief,
                                    level,
                                ),
                            }
                            cursor.show_at(surface, x, y);
                            enter(&screen, animate, &mut motion, x, y);
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
                                        if !try_transcribe_path(
                                            &mut sview,
                                            &serial_port,
                                            &mut status_buf,
                                            grants,
                                            query.as_str(),
                                        ) {
                                            sview.run_via(query.as_str(), grants);
                                            serial_port.write_str("search: ran\n");
                                        }
                                        dirty = true;
                                    } else if view == screens::View::Playbook {
                                        if playbook_step + 1 < playbook.steps.len() {
                                            playbook_step += 1;
                                            dirty = true;
                                        } else if playbook
                                            .required
                                            .map_or(true, |cap| grants.allows(cap))
                                        {
                                            // Same contract as the button: a
                                            // typed goal goes to the agent, an
                                            // empty one runs the skill itself.
                                            if !playbook_goal.is_empty() {
                                                sview.run_via(playbook_goal.as_str(), grants);
                                                view = screens::View::Search;
                                                serial_port
                                                    .write_str("playbook: approved agent run\n");
                                            } else {
                                                brief = run_skill(
                                                    status_str(
                                                        &playbook_name,
                                                        playbook_name_len,
                                                    ),
                                                    grants,
                                                    &serial_port,
                                                );
                                                view = screens::View::Brief;
                                            }
                                            dirty = true;
                                        }
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
                                // Reader navigation. A document longer than a
                                // screen was previously unreadable past line 20.
                                k if view == screens::View::Reader => {
                                    let before = scroll;
                                    scroll = match k {
                                        keyboard::Key::Down => scroll + 1,
                                        keyboard::Key::Up => scroll.saturating_sub(1),
                                        keyboard::Key::PageDown => {
                                            scroll + searchui::READER_ROWS
                                        }
                                        keyboard::Key::PageUp => {
                                            scroll.saturating_sub(searchui::READER_ROWS)
                                        }
                                        keyboard::Key::Home => 0,
                                        keyboard::Key::End => page.count,
                                        _ => scroll,
                                    };
                                    scroll = searchui::clamp_scroll(scroll, page.count);
                                    if scroll != before {
                                        dirty = true;
                                    }
                                }
                                other => {
                                    // Only the search screen has a field.
                                    // Without this, typing on Skills or
                                    // Capabilities silently built a query you
                                    // could not see.
                                    if view == screens::View::Search && query.apply(other) {
                                        dirty = true;
                                    } else if view == screens::View::Playbook
                                        && playbook_goal.apply(other)
                                    {
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
                                    scroll = 0;
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
                                            caps::Cap::EmailSearch,
                                            "email.forget",
                                            &[][..],
                                        ),
                                        (
                                            caps::Cap::WorkspaceIndex,
                                            "workspace.forget",
                                            &["workspace.index"][..],
                                        ),
                                        (caps::Cap::AudioTranscribe, "audio.forget", &[][..]),
                                        (
                                            caps::Cap::SkillsSave,
                                            "skills.forget",
                                            &[][..],
                                        ),
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
                                            } else if cap == caps::Cap::AudioTranscribe {
                                                serial_port.write_str(
                                                    "caps: granted audio.transcribe - type a media path in Search\n",
                                                );
                                            }
                                        }
                                    }
                                    // Email grant flips the home peek; refresh
                                    // after forget/grant so Recent mail matches.
                                    if before.allows(caps::Cap::EmailSearch)
                                        != grants.allows(caps::Cap::EmailSearch)
                                    {
                                        mail = mcp::fetch_mail_peek(grants);
                                    }
                                    // Your files grant flips the home peek;
                                    // refresh after index/forget so Recent files
                                    // matches the switch.
                                    if before.allows(caps::Cap::WorkspaceIndex)
                                        != grants.allows(caps::Cap::WorkspaceIndex)
                                    {
                                        files = mcp::fetch_files_peek(grants);
                                    }
                                    // Save skills revoke purges user playbooks —
                                    // refresh the Skills list so src=saved rows
                                    // do not linger after the switch flips off.
                                    if before.allows(caps::Cap::SkillsSave)
                                        && !grants.allows(caps::Cap::SkillsSave)
                                    {
                                        skill_peek = mcp::fetch_skill_peek();
                                    }
                                    // Revoking Send mail disarms any Confirm CTA.
                                    if before.allows(caps::Cap::EmailSend)
                                        && !grants.allows(caps::Cap::EmailSend)
                                    {
                                        brief.clear_send();
                                    }
                                    status_len = grants.describe(&mut status_buf);
                                    dirty = true;
                                }
                            } else if view == screens::View::Brief {
                                if let Some(ev) = screens::brief_event_hit(w, &brief, x, y) {
                                    let mut url_buf = [0u8; 40];
                                    if let Some(url) = brief.event_url_at(ev, &mut url_buf) {
                                        open_title.clear();
                                        // Title is the Event report text.
                                        if let Some(line_i) = brief.event_line_at(ev) {
                                            for b in brief.lines[line_i].text().bytes() {
                                                open_title.apply(keyboard::Key::Char(b));
                                            }
                                        }
                                        page = mcp::fetch_doc(grants, url);
                                        view = screens::View::Reader;
                                        serial_port.write_str("ui: open event\n");
                                        dirty = true;
                                    }
                                } else if let Some(di) = screens::brief_doc_hit(w, &brief, x, y) {
                                    if let Some(url) = brief.doc_url_at(di) {
                                        open_title.clear();
                                        if let Some(line_i) = brief.doc_line_at(di) {
                                            for b in brief.lines[line_i].text().bytes() {
                                                open_title.apply(keyboard::Key::Char(b));
                                            }
                                        }
                                        page = mcp::fetch_doc(grants, url);
                                        view = screens::View::Reader;
                                        serial_port.write_str("ui: open doc\n");
                                        dirty = true;
                                    }
                                } else if screens::brief_send_hit(
                                    w,
                                    h,
                                    brief.send_ready,
                                    x,
                                    y,
                                ) {
                                    match mcp::send_mail(
                                        grants,
                                        brief.draft_to(),
                                        brief.draft_subj(),
                                        "Draft from os Brief confirm",
                                    ) {
                                        mcp::SendMailStatus::Ok => {
                                            write_status(&mut status_buf, "Mail queued (mock)");
                                            serial_port.write_str("email: sent mock\n");
                                            brief.clear_send();
                                            brief.push_report("Sent", "Mock queued on the bridge");
                                        }
                                        mcp::SendMailStatus::Denied => {
                                            write_status(&mut status_buf, "Grant Send mail first");
                                            serial_port.write_str("email: send need email.send\n");
                                            brief.clear_send();
                                        }
                                        mcp::SendMailStatus::Offline => {
                                            write_status(&mut status_buf, "Bridge offline");
                                            serial_port.write_str("email: send offline\n");
                                        }
                                        mcp::SendMailStatus::Failed => {
                                            write_status(&mut status_buf, "email.send failed");
                                            serial_port.write_str("email: send failed\n");
                                        }
                                    }
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
                                    // Review first: a row opens its checklist.
                                    // Approving the last step is what actually
                                    // runs the skill (see View::Playbook).
                                    let name = skill_peek.name_at(i);
                                    playbook = skills::workflow_for(name);
                                    playbook_step = 0;
                                    playbook_goal.clear();
                                    write_status(&mut playbook_name, name);
                                    playbook_name_len =
                                        name.len().min(playbook_name.len() - 1);
                                    view = screens::View::Playbook;
                                    serial_port.write_str("ui: open playbook ");
                                    serial_port.write_str(name);
                                    serial_port.write_str("\n");
                                    dirty = true;
                                }
                            } else if view == screens::View::Playbook {
                                let (px, py, pw, ph) = screens::playbook_next_rect(
                                    w,
                                    playbook_step,
                                    playbook.steps.len(),
                                );
                                if x >= px && x < px + pw && y >= py && y < py + ph {
                                    if playbook_step + 1 < playbook.steps.len() {
                                        playbook_step += 1;
                                        dirty = true;
                                    } else if playbook.required.map_or(true, |cap| grants.allows(cap))
                                    {
                                        if !playbook_goal.is_empty() {
                                            // A typed goal is the sentence the
                                            // scoped agent should answer.
                                            sview.run_via(playbook_goal.as_str(), grants);
                                            view = screens::View::Search;
                                            serial_port
                                                .write_str("playbook: approved agent run\n");
                                        } else {
                                            // No goal typed: the approved plan is
                                            // the skill itself. Run it and report
                                            // on Brief.
                                            brief = run_skill(
                                                status_str(&playbook_name, playbook_name_len),
                                                grants,
                                                &serial_port,
                                            );
                                            view = screens::View::Brief;
                                        }
                                        dirty = true;
                                    }
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
                                    level,
                                ),
                                screens::View::Skills => {
                                    screens::draw_skills(surface, &skill_peek, grants)
                                }
                                screens::View::Playbook => {
                                    screens::draw_playbook(
                                        surface,
                                        playbook,
                                        playbook_step,
                                        &playbook_goal,
                                        caret,
                                        grants,
                                    )
                                }
                                screens::View::Caps => screens::draw_caps(surface, grants, level),
                                screens::View::Brief => screens::draw_brief(surface, &brief),
                                screens::View::Reader => {
                                    searchui::draw_reader(surface, open_title.as_str(), &page, scroll)
                                }
                                screens::View::Status => {
                                    screens::draw_status(surface, &mail, &portal, grants)
                                }
                                screens::View::Home => {
                                    ui::draw_home(
                                        surface,
                                        &mail,
                                        &files,
                                        &skill_peek,
                                        status_str(&status_buf, status_len),
                                        grants,
                                        &brief,
                                        level,
                                    )
                                }
                            }
                            cursor.show_at(surface, x, y);
                            enter(&screen, animate, &mut motion, x, y);
                            moved = false;
                        }
                    } else {
                        let left_down = buttons & 1 != 0;
                        let left_was = prev_buttons & 1 != 0;
                        if left_down && !left_was {
                            let targets = ui::home_targets(w, h, &skill_peek, &brief, &mail, &files);
                            let mut clicked = false;
                            match targets.hit(x, y) {
                                Some(ui::HomeHit::Cta(ui::CtaId::Ready)) => {
                                    serial_port.write_str("ui: click Ready\n");
                                    setup = setup::Setup::restart(level);
                                    cursor.hide(surface);
                                    setup.draw(surface, &mail, &skill_peek);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen, animate, &mut motion, x, y);
                                    moved = false;
                                    clicked = true;
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
                                    enter(&screen, animate, &mut motion, x, y);
                                    moved = false;
                                    // Don't fall through to the home redraw below.
                                    clicked = false;
                                }
                                Some(ui::HomeHit::Cta(ui::CtaId::Portal)) => {
                                    // Credentials stay with the host's Keychain. This
                                    // guest only requests consent to use that account;
                                    // it never receives or paints a password.
                                    serial_port.write_str("ui: connect tsearch account\n");
                                    status_len = grants.describe(&mut status_buf);
                                    view = screens::View::Caps;
                                    cursor.hide(surface);
                                    screens::draw_caps(surface, grants, level);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen, animate, &mut motion, x, y);
                                    moved = false;
                                    clicked = false;
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
                                        level,
                                    );
                                    cursor.show_at(surface, x, y);
                                    enter(&screen, animate, &mut motion, x, y);
                                    moved = false;
                                    clicked = true;
                                }
                                Some(ui::HomeHit::Card(ui::CardId::Capabilities)) => {
                                    serial_port.write_str("ui: click Capabilities\n");
                                    status_len = grants.describe(&mut status_buf);
                                    view = screens::View::Caps;
                                    cursor.hide(surface);
                                    screens::draw_caps(surface, grants, level);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen, animate, &mut motion, x, y);
                                    moved = false;
                                    clicked = false;
                                }
                                Some(ui::HomeHit::Brief) => {
                                    serial_port.write_str("ui: reopen brief\n");
                                    view = screens::View::Brief;
                                    cursor.hide(surface);
                                    screens::draw_brief(surface, &brief);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen, animate, &mut motion, x, y);
                                    moved = false;
                                    clicked = false;
                                }
                                Some(ui::HomeHit::Mail(i)) => {
                                    // A bridge that sends a source URL wins; the
                                    // graph id (email://{id}) is the fallback for
                                    // backends that only report an id.
                                    let mut url_buf = [0u8; 40];
                                    let direct = mail.row_url(i);
                                    let opened = if direct.is_empty() {
                                        mail.url_at(i, &mut url_buf)
                                    } else {
                                        Some(direct)
                                    };
                                    if let Some(url) = opened {
                                        open_title.clear();
                                        for b in mail.row_subj(i).bytes() {
                                            open_title.apply(keyboard::Key::Char(b));
                                        }
                                        page = mcp::fetch_doc(grants, url);
                                        scroll = 0;
                                        view = screens::View::Reader;
                                        serial_port.write_str("ui: open mail\n");
                                        cursor.hide(surface);
                                        searchui::draw_reader(
                                            surface,
                                            open_title.as_str(),
                                            &page,
                                            scroll,
                                        );
                                        cursor.show_at(surface, x, y);
                                        enter(&screen, animate, &mut motion, x, y);
                                        moved = false;
                                    } else {
                                        serial_port.write_str("ui: mail missing id\n");
                                    }
                                    clicked = false;
                                }
                                Some(ui::HomeHit::File(i)) => {
                                    let url = files.url_at(i);
                                    if !url.is_empty() {
                                        open_title.clear();
                                        for b in files.title_at(i).bytes() {
                                            open_title.apply(keyboard::Key::Char(b));
                                        }
                                        page = mcp::fetch_doc(grants, url);
                                        scroll = 0;
                                        view = screens::View::Reader;
                                        serial_port.write_str("ui: open file\n");
                                        cursor.hide(surface);
                                        searchui::draw_reader(
                                            surface,
                                            open_title.as_str(),
                                            &page,
                                            scroll,
                                        );
                                        cursor.show_at(surface, x, y);
                                        enter(&screen, animate, &mut motion, x, y);
                                        moved = false;
                                    } else {
                                        serial_port.write_str("ui: file missing url\n");
                                    }
                                    clicked = false;
                                }
                                None => {}
                            }
                            if clicked && setup.is_finished() {
                                cursor.hide(surface);
                                ui::draw_home(
                                    surface,
                                    &mail,
                                    &files,
                                    &skill_peek,
                                    status_str(&status_buf, status_len),
                                    grants,
                                    &brief,
                                    level,
                                );
                                cursor.show_at(surface, x, y);
                                enter(&screen, animate, &mut motion, x, y);
                                moved = false;
                            }
                        }
                    }
                    if buttons != prev_buttons {
                        // Never leave the cursor visually behind a click.
                        motion.snap(x, y);
                    }
                    #[cfg(target_arch = "aarch64")]
                    {
                        // The ARM input driver arrives later than the
                        // framebuffer. Re-present the software cursor at a
                        // gentle cadence so a host redraw can never erase the
                        // only visible pointer while the guest is idle.
                        arm_cursor_refresh = arm_cursor_refresh.wrapping_add(1);
                        if arm_cursor_refresh == 0 {
                            mouse::paint_pointer(surface, x, y);
                            screen.present();
                        }
                    }
                    if moved {
                        cursor.show_at(surface, x, y);
                        // hide()/show_at() marked both footprints; present()
                        // blits exactly that union and nothing else.
                        screen.present();
                    }
                    prev_buttons = buttons;

                    // --- 60 Hz chill frame ---------------------------------
                    tick = tick.wrapping_add(1);
                    let caret_now = (tick / 36) % 2 == 0; // ~1.2 Hz blink
                    let blink_changed = caret_now != caret;
                    caret = caret_now;

                    cursor.hide(surface);
                    // Soft accent breath on the nav hairline (home + search chrome).
                    if setup.is_finished()
                        && matches!(
                            view,
                            screens::View::Home
                                | screens::View::Search
                                | screens::View::Skills
                                | screens::View::Caps
                                | screens::View::Brief
                                | screens::View::Reader
                        )
                    {
                        ui::paint_chill_rule(surface, w, tick);
                    }
                    // Caret blink: redraw field views when the phase flips.
                    if blink_changed && setup.is_finished() {
                        match view {
                            screens::View::Home => {
                                ui::draw_home_full(
                                    surface,
                                    &mail,
                                    &files,
                                    &skill_peek,
                                    status_str(&status_buf, status_len),
                                    query.as_str(),
                                    caret,
                                    grants,
                                    &brief,
                                    level,
                                );
                                ui::paint_chill_rule(surface, w, tick);
                            }
                            screens::View::Search => {
                                searchui::draw(
                                    surface,
                                    &sview,
                                    query.as_str(),
                                    caret,
                                    bridge_note(&mail),
                                    level,
                                );
                                ui::paint_chill_rule(surface, w, tick);
                            }
                            _ => {}
                        }
                    }
                    if let Some((draw_x, draw_y)) = motion.step() {
                        cursor.show_at(surface, draw_x, draw_y);
                    } else {
                        cursor.show_at(surface, x, y);
                    }
                    screen.present();
                    frame_mark = anim::pace(frame_mark, FRAME_US);
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

/// Run a catalog skill under `grants` and return the Brief to show.
///
/// Builtins run their plan; anything else previews its playbook body and CALLs
/// only the tools already granted.
fn run_skill(name: &str, grants: caps::Caps, serial_port: &serial::Serial) -> agent::Brief {
    let mut brief = agent::run(name, grants);
    if !agent::is_runnable(name) {
        let mut body_buf = [0u8; 512];
        let n = mcp::fetch_skill_body(name, &mut body_buf);
        if n > 0 {
            let body = core::str::from_utf8(&body_buf[..n]).unwrap_or("");
            agent::enrich_playbook(&mut brief, grants, body);
            agent::run_playbook_allowed(&mut brief, grants, body);
            serial_port.write_str("skills: playbook ");
            serial_port.write_str(name);
            serial_port.write_str("\n");
        } else {
            brief.push_report("Info", "Playbook body unavailable.");
            serial_port.write_str("skills: brief offline ");
            serial_port.write_str(name);
            serial_port.write_str("\n");
        }
    } else {
        serial_port.write_str("agent: run ");
        serial_port.write_str(name);
        serial_port.write_str("\n");
        if brief.denied {
            serial_port.write_str("agent: need ");
            serial_port.write_str(brief.deny_name());
            serial_port.write_str("\n");
        }
    }
    brief
}

/// If `q` is a media path and Recordings is on, transcribe then search the stem.
///
/// Returns true when the path branch handled Enter (caller must not also
/// `run_via` the raw path as a keyword query).
fn try_transcribe_path(
    sview: &mut searchui::SearchView,
    serial_port: &serial::Serial,
    // Slice, not a fixed array: the status buffer grew when the status line
    // started carrying bridge/portal detail, and the two sizes drifted apart.
    status_buf: &mut [u8],
    grants: caps::Caps,
    q: &str,
) -> bool {
    if !grants.allows(caps::Cap::AudioTranscribe) || !searchui::is_media_path(q) {
        return false;
    }
    match mcp::transcribe(grants, q) {
        mcp::TranscribeStatus::Ok => {
            write_status(status_buf, "Transcribed - searching");
            serial_port.write_str("audio: transcribed\n");
            sview.run_via(searchui::media_stem(q), grants);
        }
        mcp::TranscribeStatus::Denied => {
            write_status(status_buf, "Grant Recordings first");
            serial_port.write_str("audio: need audio.transcribe\n");
        }
        mcp::TranscribeStatus::Offline => {
            write_status(status_buf, "Bridge offline");
            serial_port.write_str("audio: offline\n");
        }
        mcp::TranscribeStatus::Failed => {
            write_status(status_buf, "Transcribe failed");
            serial_port.write_str("audio: failed\n");
        }
    }
    true
}

/// Play a screen entrance: the frame is already composed in the back buffer.
///
/// Snapping between screens is what made this feel unlike a desktop; an
/// eased slide-and-fade costs a handful of blits and reads as intentional.
fn enter(screen: &fb::Screen, animate: bool, motion: &mut mouse::CursorMotion, x: i32, y: i32) {
    // A new screen is rendered at the exact pointer position; smoothing then
    // resumes only for subsequent free movement.
    motion.snap(x, y);
    if !animate {
        // Under software emulation a slide costs 11 full-screen blits — about
        // a quarter of a second — so the "polish" reads as a stutter on every
        // click. Dirty-rect present is ~600x cheaper; just show the frame.
        screen.present_all();
        return;
    }
    let mut mark = serial::rdtsc();
    for i in 0..=anim::SLIDE_IN.frames {
        let (dy, a) = anim::SLIDE_IN.at(i);
        screen.present_slide(dy, a, ui::theme::BG);
        mark = anim::pace(mark, anim::SLIDE_IN.frame_us);
    }
}

/// Is a full-screen blit cheap enough to animate with?
///
/// Measured rather than assumed: the same code should animate on hardware
/// virtualisation and stay still under TCG, without a build flag.
fn can_animate(screen: &fb::Screen) -> bool {
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = screen;
        // No cycle counter is wired on ARM yet, so treating its zero value as
        // a fast GPU would force an unjustified 60fps animation path.
        return false;
    }

    #[cfg(target_arch = "x86_64")]
    {
    let t0 = serial::rdtsc();
    screen.present_all();
    let cost = serial::rdtsc().wrapping_sub(t0);
    // A 10-frame entrance needs each blit well inside a 16ms frame. At the
    // ~1GHz the timing code assumes, that is a few million cycles.
    cost < 4_000_000
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

fn write_status(buf: &mut [u8], s: &str) {
    buf.fill(0);
    let bytes = s.as_bytes();
    let n = bytes.len().min(buf.len().saturating_sub(1));
    buf[..n].copy_from_slice(&bytes[..n]);
}

fn status_str(buf: &[u8], len: usize) -> &str {
    core::str::from_utf8(&buf[..len.min(buf.len())]).unwrap_or("")
}

/// A panic used to exit silently. Under QEMU that at least stopped the run;
/// under UTM there is no debug-exit device, so the screen simply froze with
/// nothing written anywhere. Say what happened first.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Best-effort breadcrumb on COM1 so a UTM freeze is distinguishable from
    // a triple-fault reboot (which never reaches here).
    let com1 = serial::Serial::com1();
    com1.write_str("os: PANIC ");
    if let Some(loc) = info.location() {
        com1.write_str(loc.file());
        com1.write_str(":");
        let mut n = [0u8; 12];
        com1.write_str(u32_str(&mut n, loc.line()));
    } else {
        com1.write_str("(no location)");
    }
    com1.write_str("\n");
    serial::exit_qemu(false);
}

/// Decimal, without an allocator or `write!` (which can itself panic).
fn u32_str(buf: &mut [u8; 12], mut v: u32) -> &str {
    if v == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).unwrap_or("0");
    }
    let mut tmp = [0u8; 12];
    let mut len = 0;
    while v > 0 {
        tmp[len] = b'0' + (v % 10) as u8;
        v /= 10;
        len += 1;
    }
    for i in 0..len {
        buf[i] = tmp[len - 1 - i];
    }
    core::str::from_utf8(&buf[..len]).unwrap_or("?")
}
