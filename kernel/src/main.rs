#![no_std]
#![no_main]

use core::hint::black_box;

#[allow(unused_imports)]
use kernel::{
    acpi, agent, anim, arm64_mmio, beep, boot_splash, caps, copy, fault, fb, hello_message, inputdiag, keyboard, level,
    mcp, mouse, ohci, pci, screens, searchui, serial, setup, skills, time, ui, usb_tablet,
};
use limine::BaseRevision;
use limine::request::{
    ExecutableCmdlineRequest, FramebufferRequest, HhdmRequest, MemoryMapRequest,
    RequestsEndMarker, RequestsStartMarker, RsdpRequest, StackSizeRequest,
};

const STACK_SIZE: u64 = 128 * 1024;
/// Chill game loop: always paced at 60 Hz so ambient motion keeps breathing
/// even when the pointer is still.
const FRAME_US: u32 = anim::FRAME_US_60;

/// The small interactive region currently under the raw pointer.
///
/// Semantic targets keep hover independent from cursor smoothing: clicks and
/// feedback both follow the device coordinates immediately, while the arrow
/// can still ease between those points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HoverTarget {
    None,
    StatusDot,
    Home(ui::HomeHit),
    Back,
    SearchResult(usize),
    ScreenRow(usize),
}

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
#[unsafe(link_section = ".requests")]
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static CMDLINE_REQUEST: ExecutableCmdlineRequest = ExecutableCmdlineRequest::new();

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
    black_box(&RSDP_REQUEST);
    black_box(&CMDLINE_REQUEST);

    if !BASE_REVISION.is_supported() {
        serial::exit_qemu(false);
    }

    // MMIO is not part of Limine's higher-half RAM map. Install the small
    // identity-mapped device aperture before probing either PL011; otherwise
    // the very first UART identification read can take a synchronous abort
    // before fault vectors or the framebuffer exist.
    #[cfg(target_arch = "aarch64")]
    let _ = arm64_mmio::install_low_device_window();

    // Find the UART before anything logs through it. QEMU and VirtualBox put
    // their PL011 at different addresses; assuming QEMU's meant every line the
    // ARM guest wrote went into unmapped space. `detect_pl011` proves each
    // candidate translates before reading it, so a wrong guess is a failed
    // probe rather than a silent data abort. See `serial::detect_pl011`.
    serial::detect_pl011(HHDM_REQUEST.get_response().map(|r| r.offset()).unwrap_or(0));

    let serial_port = serial::Serial::com1();
    serial_port.init();

    // Before anything can fault. Without this a bad pointer or an overflow
    // check triple-faults and the machine silently resets, which is
    // indistinguishable from "it just randomly crashes".
    fault::init();

    serial_port.write_str(hello_message());
    serial_port.write_str(serial::LINE_ENDING);

    // Paint UI immediately (don't block on MCP). Bridge is optional. `mail` is
    // deferred rather than pre-filled with an Offline placeholder: nothing
    // paints before the boot splash now, and the splash probes the bridge, so
    // the first value it takes is the real one.
    let mut mail;
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

                // Nothing paints Home before setup has had its say. The boot
                // splash below fills the surface on its own frame 0, so the
                // firmware screen is gone just as fast without ever showing a
                // screen this machine has not earned yet. See the comment on
                // the Welcome paint after the splash.

                // ── Boot splash ──────────────────────────────────────────
                // Play the connection animation while the bridge probe runs.
                // The probe is slow (serial round-trip with a timeout), so we
                // fire it at the halfway point: frames 0..HALF play first,
                // then the probe runs, then frames HALF..TOTAL finish.
                const HALF: u32 = boot_splash::TOTAL_FRAMES / 2;
                let mut splash_tick = serial::rdtsc();
                // Phase 1: first half of animation
                for frame in 0..HALF {
                    boot_splash::draw_frame(surface, frame);
                    surface.clear_dirty();
                    screen.present();
                    splash_tick = anim::pace(splash_tick, FRAME_US);
                }
                // Run the bridge probe while the animation would show its midpoint.
                serial_port.write_str("mcp: probing bridge\n");
                mail = mcp::MailPeek::empty(mcp::probe_bridge());
                serial_port.write_str("mcp: probe returned\n");
                match mail.status {
                    mcp::BridgeStatus::Online => serial_port.write_str("mcp: bridge connected\n"),
                    mcp::BridgeStatus::Offline => serial_port.write_str("mcp: bridge offline\n"),
                }
                // Phase 2: second half (now colour-coded by bridge result)
                for frame in HALF..boot_splash::TOTAL_FRAMES {
                    boot_splash::draw_frame(surface, frame);
                    surface.clear_dirty();
                    screen.present();
                    splash_tick = anim::pace(splash_tick, FRAME_US);
                }
                // Fade out — draw one blank frame to clear the splash bg.
                surface.fill(crate::ui::theme::BG);
                screen.present();

                // Liveness only until the user consents. Reading the inbox
                // here would fetch — and, because the bridge indexes results,
                // persist to disk — mail before anyone agreed to it.
                let mut grants = caps::Caps::none();
                let mut level = level::Level::Guided;
                serial_port.write_str("skills: builtins ready\n");

                // Setup owns the screen from here to the moment it finishes.
                //
                // This used to paint Home three times on the way to the main
                // loop, and Home is a lie until setup has run: it offers a
                // query box and capability cards under `Caps::none()`, before
                // anyone has consented to anything. On x86 the window was a
                // few milliseconds and nobody saw it. On arm64 the USB probe
                // below takes over a second, so the guest showed a complete
                // Home screen and then replaced it with Welcome — which is
                // also why the e2e harness reported that Enter on Home went
                // back to the welcome screen. It never left setup; the harness
                // photographed the pre-setup Home paint and typed into it.
                let mut setup = setup::Setup::new();
                setup.draw(surface, &mail, &skill_peek);

                let cx = surface.width() as i32 / 2;
                let cy = surface.height() as i32 / 2;
                mouse::paint_pointer(surface, cx, cy);
                screen.present();
                serial_port.write_str("mouse: pointer painted\n");

                serial::request_qemu_exit(true);

                let hhdm = HHDM_REQUEST.get_response().map(|r| r.offset()).unwrap_or(0);
                #[cfg(target_arch = "aarch64")]
                if let (Some(rsdp), Some(mmap)) = (
                    RSDP_REQUEST.get_response(),
                    MEMORY_MAP_REQUEST.get_response(),
                ) {
                    let vbox = serial::is_virtualbox_arm();
                    let mmio_ready = !vbox || arm64_mmio::install_low_device_window();
                    let ecam = if !mmio_ready {
                        None
                    } else if vbox {
                        Some(acpi::VBOX_ARM_ECAM)
                    } else {
                        unsafe { acpi::find_ecam(rsdp.address(), hhdm) }
                    };
                    if let Some(ecam) = ecam {
                        // VirtualBox's ARM platform exposes device MMIO in the
                        // identity map (the PL011 uses the same arrangement).
                        // Limine's HHDM is for RAM; biasing ECAM through it
                        // addresses unrelated memory and stalls the PCI probe.
                        let ecam_virt = if serial::is_virtualbox_arm() {
                            ecam.base as usize
                        } else {
                            hhdm.saturating_add(ecam.base) as usize
                        };
                        if ecam.segment == 0
                            && acpi::mmap_covers(mmap, ecam.base, acpi::ecam_span(&ecam))
                            && pci::use_ecam(ecam_virt, ecam.start_bus, ecam.end_bus)
                        {
                            serial_port.write_str("pci: ACPI ECAM ready\n");
                        } else {
                            serial_port.write_str("pci: ECAM unavailable\n");
                        }
                    } else {
                        serial_port.write_str("pci: ACPI MCFG missing\n");
                    }
                }
                serial_port.write_str("input: probing usb\n");
                let mut ohci_input = None;
                let mut tablet = None;
                let mut why = [0u8; 24];
                let mut ohci_why = [0u8; 24];
                if let Some(mmap) = MEMORY_MAP_REQUEST.get_response() {
                    if let Some((p0, p1)) = usb_tablet::alloc_dma_pages(mmap) {
                        ohci_input = unsafe { ohci::OhciInput::init(hhdm, p0, &mut ohci_why) };
                        // The two drivers use the same small DMA reservation.
                        // VirtualBox ARM presents OHCI; UTM/QEMU presents
                        // UHCI. Only try the second path when the first did not
                        // claim a device.
                        if ohci_input.is_none() {
                            tablet = unsafe { usb_tablet::UsbTablet::init(hhdm, p0, p1, &mut why) };
                        }
                    } else {
                        let m = b"no-dma";
                        why[..m.len()].copy_from_slice(m);
                        ohci_why[..m.len()].copy_from_slice(m);
                    }
                } else {
                    let m = b"no-mmap";
                    why[..m.len()].copy_from_slice(m);
                    ohci_why[..m.len()].copy_from_slice(m);
                }
                if let Some(ref input) = ohci_input {
                    serial_port.write_str("input: OHCI ready");
                    if input.has_keyboard() {
                        serial_port.write_str(" keyboard");
                    }
                    if input.has_pointer() {
                        serial_port.write_str(" pointer");
                    }
                    serial_port.write_str("\n");
                } else {
                    serial_port.write_str("input: OHCI missing ");
                    let n = ohci_why
                        .iter()
                        .position(|&b| b == 0)
                        .unwrap_or(ohci_why.len());
                    serial_port.write_bytes(&ohci_why[..n]);
                    serial_port.write_str("\n");
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
                let mut cursor = mouse::Cursor::new();
                let mut x = cx;
                let mut y = cy;
                let mut motion = mouse::CursorMotion::new(x, y);
                let mut frame_mark = serial::rdtsc();
                let mut prev_buttons = 0u8;
                #[cfg(target_arch = "aarch64")]
                let mut arm_cursor_refresh = 0u16;
                #[cfg(target_arch = "aarch64")]
                let ui_clock = time::Timebase::probe();
                // What actually came up. Under QEMU this is always fine; on
                // real hardware it is the whole story, and a machine with no
                // driveable pointer renders a perfect home screen with a
                // cursor that never moves - indistinguishable from a hang.
                let inputs = inputdiag::Inputs {
                    ps2_controller: keyboard::Keyboard::present(),
                    ps2_keyboard: keyboard::Keyboard::present(),
                    ps2_mouse: mice.present,
                    usb_keyboard: ohci_input
                        .as_ref()
                        .is_some_and(|input| input.has_keyboard()),
                    usb_tablet: tablet.is_some()
                        || ohci_input.as_ref().is_some_and(|input| input.has_pointer()),
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

                // Say it on the screen setup is already showing.
                //
                // This used to paint the note onto Home and present it, which
                // achieved nothing: setup drew over it a moment later, so the
                // one message explaining why nothing responds was never
                // actually readable on first boot. A machine with no keyboard
                // is stuck on Welcome, so Welcome is where the note belongs.
                setup.set_note(inputs.note());

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
                let mut portal = mcp::PortalStatus {
                    reachable: false,
                    cached: false,
                    syncing: false,
                    docs: 0,
                };
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
                let mut hover = HoverTarget::None;
                let mut hover_view = view;
                let mut hover_pressed = false;
                // The hidden portal screen. Nothing on Home points at it; the
                // Ctrl+Shift+P chord is the whole entrance.
                let mut portal_cfg = screens::PortalConfig::new();
                let mut tick: u32 = 0;
                let mut caret = true;
                // First boot: run the setup journey before the home screen.
                cursor.hide(surface);
                setup.draw(surface, &mail, &skill_peek);
                cursor.show_at(surface, x, y);
                enter(&screen, animate, &mut motion, x, y);
                // Re-establish the cursor on the final transition frame. The
                // copy taken before `enter` describes a frame that no longer
                // exists; `show_at` notices the surface was drawn on and takes
                // a fresh one rather than restoring the stale page.
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
                    if let Some(ref mut input) = ohci_input {
                        if input.has_pointer() && input.poll(w, h) {
                            let point = input.pointer();
                            x = point.x.clamp(0, w.saturating_sub(1));
                            y = point.y.clamp(0, h.saturating_sub(1));
                            buttons = point.buttons;
                            mice.x = x;
                            mice.y = y;
                            mice.buttons = buttons;
                            moved = true;
                        }
                    }
                    if !moved {
                        if let Some(ref mut t) = tablet {
                            if t.poll(w, h) {
                                x = t.x.clamp(0, w.saturating_sub(1));
                                y = t.y.clamp(0, h.saturating_sub(1));
                                buttons = t.buttons;
                                mice.x = x;
                                mice.y = y;
                                mice.buttons = buttons;
                                moved = true;
                            }
                        }
                    }
                    if !moved {
                        if mice.poll(w, h) {
                            x = mice.x.clamp(0, w.saturating_sub(1));
                            y = mice.y.clamp(0, h.saturating_sub(1));
                            buttons = mice.buttons;
                            moved = true;
                        }
                    }
                    // `prev_buttons` is what this frame's buttons are compared
                    // against, so it may only be advanced after the handlers
                    // have run — it is, at the bottom of the loop. Updating it
                    // here as well made `was_down` equal `left_down` for every
                    // handler below, so no press ever read as a new press:
                    // Back, the home tiles, Brief Doc rows, Search results and
                    // the capability switches all stopped responding to the
                    // mouse at once. Setup still worked, because
                    // `setup.pointer` takes the button state rather than its
                    // edge — which is why the machine looked clickable right
                    // up to the moment setup ended.
                    if moved {
                        motion.set_target(x, y);
                    }

                    if !setup.is_finished() {
                        let hover_changed = moved && setup.pointer_hover(x, y);
                        let before = setup.step;
                        let mut setup_changed = setup.pointer(x, y, buttons);
                        while let Some(key) = poll_key(&mut ohci_input, &mut kb) {
                            setup_changed |= setup.key(key);
                        }
                        if setup_changed {
                            // Setup step changed. Move forward.
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
                        } else if hover_changed {
                            // Hover is feedback, not navigation: repaint the
                            // same setup step without replaying its entrance.
                            cursor.hide(surface);
                            setup.draw(surface, &mail, &skill_peek);
                        }
                    } else if view == screens::View::Home {
                        // The nav status affordance is the only way to reach
                        // this screen, which shows every source's state — more
                        // than a dot (or a word) can express on its own. Ask
                        // `ui` where it drew that affordance rather than
                        // assuming the dot: a standalone build draws a word.
                        let left_down = buttons & 0x01 != 0;
                        let was_down = prev_buttons & 0x01 != 0;
                        if left_down && !was_down && ui::status_hit_rect(w).contains(x, y) {
                            portal = mcp::portal_status();
                            view = screens::View::Status;
                            cursor.hide(surface);
                            screens::draw_status(surface, &mail, &portal, grants);
                            cursor.show_at(surface, x, y);
                            enter(&screen, animate, &mut motion, x, y);
                        }

                        // Type straight into the home field - no click first.
                        let mut dirty = false;
                        // True when the ONLY thing that changed is the query text. Typing must
                        // not repaint the whole screen: draw_home_full opens with fill(BG), and a
                        // full present costs ~22.5ms over emulated MMIO, which is visible as a
                        // flicker on every keypress.
                        let mut query_only = false;
                        while let Some(key) = poll_key(&mut ohci_input, &mut kb) {
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
                                            query_only = false;
                                            dirty = true;
                                        } else {
                                            // Agentic home: plan/act under caps,
                                            // then a Brief with openable Doc rows.
                                            brief = agent::run_goal(query.as_str(), grants);
                                            view = screens::View::Brief;
                                            query_only = false;
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
                                // The only door into the portal screen. There
                                // is deliberately no visible affordance for
                                // this — the machine ships handed to a friend,
                                // and this is the setter-upper's way back in.
                                keyboard::Key::Chord(b'P') => {
                                    // Unlock is per-connection on the host, so
                                    // read the real state rather than trusting
                                    // whatever this screen believed last time.
                                    portal_cfg.refresh(mcp::config_status());
                                    view = screens::View::PortalConfig;
                                    query_only = false;
                                    serial_port.write_str("ui: config screen\n");
                                    dirty = true;
                                }
                                other => {
                                    if query.apply(other) {
                                        dirty = true;
                                        query_only = true;
                                    }
                                }
                            }
                        }
                        if dirty {
                            if !query_only {
                                // A full redraw may cover an existing intent
                                // rail. Re-derive it from the live pointer in
                                // the common frame instead of trusting stale
                                // painted state. The bounded field painter
                                // leaves the existing rail untouched.
                                hover = HoverTarget::None;
                                hover_pressed = false;
                            }
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
                                screens::View::PortalConfig => {
                                    screens::draw_portal_config(surface, &portal_cfg, caret)
                                }
                                screens::View::Reader => {
                                    searchui::draw_reader(
                                        surface,
                                        open_title.as_str(),
                                        &page,
                                        scroll,
                                    )
                                }
                                // Typing changes one rectangle. Repainting
                                // the screen for it clears and rewrites every
                                // pixel, which at ~22.5ms a present is visible
                                // as a flicker on each keypress.
                                _ if query_only => ui::draw_search_field(
                                    surface,
                                    w,
                                    h,
                                    query.as_str(),
                                    caret,
                                    level,
                                ),
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
                            if query_only {
                                // Typing is an in-place edit, not a screen
                                // transition. Sliding the whole framebuffer
                                // for one changed field made each key feel like
                                // opening a new page.
                                screen.present();
                            } else {
                                enter(&screen, animate, &mut motion, x, y);
                            }
                        }
                    }
                    if view != screens::View::Home {
                        // --- search screen: keyboard drives it ---
                        let mut dirty = false;
                        let mut query_only = false;
                        let mut full_redraw = false;
                        while let Some(key) = poll_key(&mut ohci_input, &mut kb) {
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
                                        full_redraw = true;
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
                                    } else if view == screens::View::PortalConfig
                                        && portal_cfg.status.locked
                                    {
                                        // The secret goes to COM2 and nowhere
                                        // else: `submit` borrows it for the
                                        // call, so it never reaches a variable
                                        // here that could end up in the log.
                                        serial_port.write_str("ui: config unlock attempt\n");
                                        let reply = portal_cfg.submit();
                                        serial_port.write_str(match reply {
                                            mcp::UnlockStatus::Ok => "config: unlocked\n",
                                            mcp::UnlockStatus::BadPass => "config: unlock refused\n",
                                            mcp::UnlockStatus::NotConfigured => {
                                                "config: no password set on the host\n"
                                            }
                                            mcp::UnlockStatus::TooMany => {
                                                "config: unlock rate limited\n"
                                            }
                                            mcp::UnlockStatus::Offline => "config: bridge offline\n",
                                            // Refused before COM2 was opened.
                                            mcp::UnlockStatus::Unsendable => {
                                                "config: password not frameable - not sent\n"
                                            }
                                        });
                                        dirty = true;
                                    }
                                }
                                keyboard::Key::Escape => {
                                    if view == screens::View::PortalConfig {
                                        // Leaving drops anything half-typed.
                                        portal_cfg.refresh(mcp::ConfigStatus::offline());
                                    }
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
                                        keyboard::Key::PageDown => scroll + searchui::READER_ROWS,
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
                                        query_only = !full_redraw;
                                    } else if view == screens::View::Playbook
                                        && playbook_goal.apply(other)
                                    {
                                        dirty = true;
                                    } else if view == screens::View::PortalConfig
                                        && portal_cfg.status.locked
                                        && portal_cfg.type_key(other)
                                    {
                                        // Keystrokes land in the field, never
                                        // in the serial log.
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
                                if view == screens::View::PortalConfig {
                                    // Leaving drops anything half-typed.
                                    portal_cfg.refresh(mcp::ConfigStatus::offline());
                                }
                                // Back from the reader returns to results.
                                view = if view == screens::View::Reader {
                                    screens::View::Search
                                } else {
                                    screens::View::Home
                                };
                                dirty = true;
                            } else if view == screens::View::PortalConfig {
                                // Only the unlocked picker has rows to click.
                                if !portal_cfg.status.locked {
                                    if let Some(i) = screens::portal_family_hit(w, x, y) {
                                        if let Some(family) =
                                            mcp::PortalFamily::ALL.get(i).copied()
                                        {
                                            serial_port.write_str("ui: config portal ");
                                            serial_port.write_str(family.wire());
                                            serial_port.write_str("\n");
                                            let reply = mcp::config_portal(family);
                                            portal_cfg.apply_portal(reply);
                                            serial_port.write_str(match reply {
                                                mcp::PortalSetStatus::Ok(_) => "config: saved\n",
                                                // Per-connection unlock lost;
                                                // the screen has already fallen
                                                // back to the password.
                                                mcp::PortalSetStatus::Locked => {
                                                    "config: session relocked\n"
                                                }
                                                mcp::PortalSetStatus::UnknownFamily => {
                                                    "config: host rejected the family\n"
                                                }
                                                mcp::PortalSetStatus::Offline => {
                                                    "config: bridge offline\n"
                                                }
                                                mcp::PortalSetStatus::Failed => {
                                                    "config: portal set failed\n"
                                                }
                                            });
                                            dirty = true;
                                        }
                                    }
                                }
                            } else if view == screens::View::Search {
                                // Open a result.
                                if let Some(i) = searchui::result_hit(w, h, sview.count, x, y) {
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
                                            brief.push_report("Sent", copy::mail_queued_mock());
                                        }
                                        mcp::SendMailStatus::Denied => {
                                            write_status(&mut status_buf, "Grant Send mail first");
                                            serial_port.write_str("email: send need email.send\n");
                                            brief.clear_send();
                                        }
                                        mcp::SendMailStatus::Offline => {
                                            write_status(&mut status_buf, copy::status_offline());
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
                                            write_status(&mut status_buf, copy::status_offline());
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
                            if !query_only || full_redraw {
                                // Full secondary-screen redraws replace the
                                // rail. The common frame below reinstates the
                                // right one; field-only edits preserve it.
                                hover = HoverTarget::None;
                                hover_pressed = false;
                            }
                            cursor.hide(surface);
                            match view {
                                screens::View::Search if query_only && !full_redraw => {
                                    searchui::draw_search_field(
                                        surface,
                                        w,
                                        h,
                                        query.as_str(),
                                        caret,
                                        level,
                                    )
                                }
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
                                screens::View::PortalConfig => {
                                    screens::draw_portal_config(surface, &portal_cfg, caret)
                                }
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
                            if query_only && !full_redraw {
                                screen.present();
                            } else {
                                enter(&screen, animate, &mut motion, x, y);
                            }
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
                                    setup = setup::Setup::restart(level, inputs.note());
                                    cursor.hide(surface);
                                    setup.draw(surface, &mail, &skill_peek);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen, animate, &mut motion, x, y);
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
                                    // Don't fall through to the home redraw below.
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
                                    // Search is already composed. Falling into
                                    // the shared Home redraw leaves `view`
                                    // saying Search while the framebuffer
                                    // visibly shows Home.
                                    clicked = false;
                                }
                                Some(ui::HomeHit::Card(ui::CardId::Capabilities)) => {
                                    serial_port.write_str("ui: click Capabilities\n");
                                    status_len = grants.describe(&mut status_buf);
                                    view = screens::View::Caps;
                                    cursor.hide(surface);
                                    screens::draw_caps(surface, grants, level);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen, animate, &mut motion, x, y);
                                    clicked = false;
                                }
                                Some(ui::HomeHit::Brief) => {
                                    serial_port.write_str("ui: reopen brief\n");
                                    view = screens::View::Brief;
                                    cursor.hide(surface);
                                    screens::draw_brief(surface, &brief);
                                    cursor.show_at(surface, x, y);
                                    enter(&screen, animate, &mut motion, x, y);
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
                        // Through the cursor, not `paint_pointer`: a direct
                        // paint keeps no saved background yet still counts as
                        // drawing, so the next move finds its copy stale,
                        // declines to restore, and strands the arrow. Repeated
                        // at the start position, that is exactly the ghost
                        // that sat in the middle of the screen until something
                        // repainted the whole frame.
                        arm_cursor_refresh = arm_cursor_refresh.wrapping_add(1);
                        if arm_cursor_refresh == 0 {
                            cursor.show_at(surface, x, y);
                            screen.present();
                        }
                    }
                    // ARM's frame pacing below is derived from CNTFRQ_EL0, but
                    // the timebase is the independent floor: a guest whose
                    // counter reads badly must still yield rather than spin
                    // the whole loop flat out.
                    #[cfg(target_arch = "aarch64")]
                    time::delay_ms(&ui_clock, 1);
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
                                | screens::View::PortalConfig
                        )
                    {
                        ui::paint_chill_rule(surface, w, tick);
                    }
                    // Caret blink: redraw field views when the phase flips.
                    if blink_changed && setup.is_finished() {
                        match view {
                            screens::View::Home => {
                                // Only the caret changed. Repainting the whole
                                // screen for it cost a ~22.5ms full present
                                // twice a second and made the display visibly
                                // flicker while it was being rewritten.
                                ui::draw_search_field(
                                    surface,
                                    w,
                                    h,
                                    query.as_str(),
                                    caret,
                                    level,
                                );
                            }
                            screens::View::Search => {
                                // Results and the agent sentence did not
                                // change; keep a blink to one bounded field.
                                searchui::draw_search_field(
                                    surface,
                                    w,
                                    h,
                                    query.as_str(),
                                    caret,
                                    level,
                                );
                            }
                            // The password field has a caret too; without this
                            // it would sit frozen while every other field
                            // blinks.
                            screens::View::PortalConfig if portal_cfg.status.locked => {
                                screens::draw_portal_config(surface, &portal_cfg, caret);
                                ui::paint_chill_rule(surface, w, tick);
                                hover = HoverTarget::None;
                                hover_pressed = false;
                            }
                            _ => {}
                        }
                    }

                    // Derive intent from the raw pointer once per frame. A
                    // target transition repaints two 2px rails; staying inside
                    // the same target performs no UI work. View changes drop
                    // the old rail without erasing it over the freshly drawn
                    // screen.
                    let home_targets =
                        ui::home_targets(w, h, &skill_peek, &brief, &mail, &files);
                    let (bx, by, bw, bh) = searchui::back_rect(w);
                    let over_back = x >= bx && x < bx + bw && y >= by && y < by + bh;
                    let next_hover = if !setup.is_finished() {
                        HoverTarget::None
                    } else {
                        match view {
                            screens::View::Home => {
                                if ui::status_hit_rect(w).contains(x, y) {
                                    HoverTarget::StatusDot
                                } else {
                                    home_targets
                                        .hit(x, y)
                                        .map(HoverTarget::Home)
                                        .unwrap_or(HoverTarget::None)
                                }
                            }
                            screens::View::Search => {
                                if over_back {
                                    HoverTarget::Back
                                } else {
                                    searchui::result_hit(w, h, sview.count, x, y)
                                        .map(HoverTarget::SearchResult)
                                        .unwrap_or(HoverTarget::None)
                                }
                            }
                            screens::View::Caps => {
                                if over_back {
                                    HoverTarget::Back
                                } else {
                                    screens::caps_hit(w, x, y)
                                        .map(HoverTarget::ScreenRow)
                                        .unwrap_or(HoverTarget::None)
                                }
                            }
                            screens::View::Skills => {
                                if over_back {
                                    HoverTarget::Back
                                } else if screens::skills_save_hit(
                                    w,
                                    skill_peek.count,
                                    grants.allows(caps::Cap::SkillsSave),
                                    x,
                                    y,
                                ) {
                                    HoverTarget::ScreenRow(skill_peek.count.min(7))
                                } else {
                                    screens::skills_hit(w, skill_peek.count, x, y)
                                        .map(HoverTarget::ScreenRow)
                                        .unwrap_or(HoverTarget::None)
                                }
                            }
                            screens::View::PortalConfig => {
                                if over_back {
                                    HoverTarget::Back
                                } else if !portal_cfg.status.locked {
                                    screens::portal_family_hit(w, x, y)
                                        .map(HoverTarget::ScreenRow)
                                        .unwrap_or(HoverTarget::None)
                                } else {
                                    HoverTarget::None
                                }
                            }
                            screens::View::Reader
                            | screens::View::Brief
                            | screens::View::Status
                            | screens::View::Playbook => {
                                if over_back {
                                    HoverTarget::Back
                                } else {
                                    HoverTarget::None
                                }
                            }
                        }
                    };
                    let next_pressed =
                        next_hover != HoverTarget::None && buttons & 0x01 != 0;
                    let same_view = setup.is_finished() && hover_view == view;
                    if !same_view {
                        hover = HoverTarget::None;
                        hover_pressed = false;
                        hover_view = view;
                    }
                    if hover != next_hover || hover_pressed != next_pressed {
                        if same_view && hover != HoverTarget::None {
                            paint_hover(
                                surface,
                                view,
                                hover,
                                false,
                                false,
                                w,
                                h,
                                home_targets,
                            );
                        }
                        if next_hover != HoverTarget::None {
                            paint_hover(
                                surface,
                                view,
                                next_hover,
                                true,
                                next_pressed,
                                w,
                                h,
                                home_targets,
                            );
                        }
                        hover = next_hover;
                        hover_pressed = next_pressed;
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

/// Paint one bounded interaction rail.
///
/// Hover is a pale intent cue; holding the primary button deepens it to the
/// normal action blue. The shape never grows, so press feedback cannot trigger
/// layout or a larger framebuffer transfer.
fn paint_hover(
    fb: &fb::Surface,
    view: screens::View,
    target: HoverTarget,
    on: bool,
    pressed: bool,
    w: i32,
    h: i32,
    home: ui::HomeTargets,
) {
    let rect = match (view, target) {
        (screens::View::Home, HoverTarget::StatusDot) => Some(ui::status_hover_rect(w)),
        (screens::View::Home, HoverTarget::Home(hit)) => Some(ui::home_hover_rect(home, hit)),
        (screens::View::Search, HoverTarget::SearchResult(i)) => {
            Some(searchui::result_hover_rect(w, h, i))
        }
        (
            screens::View::Caps | screens::View::Skills | screens::View::PortalConfig,
            HoverTarget::ScreenRow(i),
        ) => Some(screens::row_hover_rect(w, i)),
        (
            screens::View::Search
            | screens::View::Skills
            | screens::View::Caps
            | screens::View::Reader
            | screens::View::Brief
            | screens::View::Status
            | screens::View::Playbook
            | screens::View::PortalConfig,
            HoverTarget::Back,
        ) => Some(searchui::back_hover_rect(w)),
        _ => None,
    };
    if let Some(rect) = rect.filter(|r| r.w > 0 && r.h > 0) {
        fb.fill_rect(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            if !on {
                ui::theme::BG
            } else if pressed {
                ui::theme::ACCENT
            } else {
                ui::theme::TINT_BORDER
            },
        );
    }
}

/// Drain the native USB keyboard before falling back to PS/2/serial.
///
/// Both sources produce the same `Key`, so the UI does not care which
/// controller delivered it and ARM does not need a parallel event path.
fn poll_key(
    usb: &mut Option<ohci::OhciInput>,
    fallback: &mut keyboard::Keyboard,
) -> Option<keyboard::Key> {
    usb.as_mut()
        .and_then(ohci::OhciInput::next_key)
        .or_else(|| fallback.poll())
}

/// One line telling the user where answers come from right now.
///
/// The offline half lives in `copy` with every other sentence a standalone
/// image can print. This one was written inline and kept saying "Bridge
/// offline" above the search field of a machine that has no bridge to bring
/// up — the first line a new owner reads, naming a repair they cannot make.
fn bridge_note(mail: &mcp::MailPeek) -> &'static str {
    match mail.status {
        mcp::BridgeStatus::Online => {
            if mcp::standalone() {
                // Standalone never attaches a host; Online would be a bug, but
                // the copy still must not invent a bridge for the owner.
                "Answers come from the index built into this device."
            } else {
                "Answers come from the local index and the host bridge."
            }
        }
        mcp::BridgeStatus::Offline => copy::search_source_note(),
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
            write_status(status_buf, copy::status_offline());
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
        // click. Full-screen transitions already dirty the whole surface;
        // field and hover updates do not, so preserve their bounded blits.
        screen.present();
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
    #[cfg(target_arch = "aarch64")]
    {
        let hz = serial::counter_hz();
        if hz == 0 || !screen.is_buffered() {
            return false;
        }
        let t0 = serial::rdtsc();
        screen.present_all();
        let ticks = serial::rdtsc().wrapping_sub(t0);
        // Leave half of a 16.7 ms frame for composition and input. A slower
        // RamFB still gets instant dirty-rect screen changes, not stutter.
        return ticks.saturating_mul(1_000_000) / hz < 8_000;
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
