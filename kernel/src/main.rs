#![no_std]
#![no_main]

use core::hint::black_box;

use kernel::{caps, fb, hello_message, mcp, mouse, serial, setup, skills, ui, usb_tablet};
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

                // Early peek uses default grants so smoke still exercises COM2
                // before the setup journey runs (smoke exits before setup).
                let mut grants = caps::Caps::default_grants();
                mail = mcp::fetch_mail_peek(grants);
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
                let (mut prev_x, mut prev_y) = (cx, cy);
                let mut status_buf = [0u8; 72];
                write_status(&mut status_buf, grants.footer_status());
                let mut setup = setup::Setup::new();
                // First boot: run the setup journey before the home screen.
                cursor.hide(surface);
                setup.draw(surface, &mail, &skill_peek);
                cursor.show_at(surface, x, y);
                serial_port.write_str("ui: setup welcome\n");

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
                                mail = mcp::fetch_mail_peek(setup.grants());
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
                            screen.present();
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
                                moved = false;
                            }
                        }
                    }
                    prev_buttons = buttons;
                    if moved {
                        cursor.show_at(surface, x, y);
                        // Blit only the two cursor footprints, not the screen.
                        const PAD: i32 = 40;
                        screen.present_rect(prev_x - 2, prev_y - 2, PAD, PAD);
                        screen.present_rect(x - 2, y - 2, PAD, PAD);
                        prev_x = x;
                        prev_y = y;
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
