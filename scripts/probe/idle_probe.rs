// Scratch probe for ci/idle-vblank-ab, built as a gpui example. Not for main.
//
// `idle_probe idle`  shows a window and does nothing.
// `idle_probe clock` shows a window that redraws once a second (a recording timer).
// `idle_probe check` checks that an idle window still repaints on demand, runs
//                    animations, delivers next-frame callbacks and repaints on
//                    resize, then exits 0 (or 1 on the first failure).

use std::{
    cell::Cell,
    io::Write as _,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    App, AsyncWindowContext, Bounds, Context, Pixels, Size, WeakEntity, Window, WindowBounds,
    WindowOptions, div, point, prelude::*, px, rgb, size,
};
use gpui_platform::application;

const COLORS: [u32; 6] = [0xd03030, 0x30a040, 0x3050d0, 0xd0a020, 0x9040b0, 0x20b0b0];

struct Probe {
    color: u32,
    renders: Rc<Cell<u64>>,
    viewport: Rc<Cell<Size<Pixels>>>,
    animate_until: Option<Instant>,
}

impl Render for Probe {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        self.viewport.set(window.viewport_size());
        if let Some(until) = self.animate_until {
            if Instant::now() < until {
                window.request_animation_frame();
            } else {
                self.animate_until = None;
            }
        }
        div().size_full().bg(rgb(self.color))
    }
}

fn say(line: String) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn fail(what: String) -> ! {
    say(format!("CHECK FAIL {what}"));
    std::process::exit(1);
}

async fn sleep(cx: &mut AsyncWindowContext, ms: u64) {
    cx.background_executor()
        .timer(Duration::from_millis(ms))
        .await;
}

/// Polls until `done` holds, failing after `limit_ms`. Returns the wait in ms.
async fn wait_for(
    cx: &mut AsyncWindowContext,
    what: &str,
    limit_ms: u64,
    done: impl Fn() -> bool,
) -> f64 {
    let start = Instant::now();
    while !done() {
        if start.elapsed() > Duration::from_millis(limit_ms) {
            fail(format!("{what}: nothing within {limit_ms} ms"));
        }
        sleep(cx, 2).await;
    }
    start.elapsed().as_secs_f64() * 1000.0
}

async fn check(
    this: WeakEntity<Probe>,
    renders: Rc<Cell<u64>>,
    viewport: Rc<Cell<Size<Pixels>>>,
    cx: &mut AsyncWindowContext,
) {
    // Long past the pacer's idle delay, so every step starts from a stopped window.
    sleep(cx, 1500).await;

    // 1. A notify on an idle window draws a frame, and the frame reaches the screen.
    for (index, color) in COLORS.iter().copied().enumerate() {
        let before = renders.get();
        this.update(cx, |this, cx| {
            this.color = color;
            cx.notify();
        })
        .unwrap_or_else(|_| fail("window gone".into()));
        let latency = wait_for(cx, &format!("notify {index}"), 1000, || {
            renders.get() > before
        })
        .await;
        let (bounds, scale) = cx
            .update(|window, _| (window.bounds(), window.scale_factor()))
            .unwrap_or_else(|_| fail("window gone".into()));
        say(format!(
            "COLOR {color:06x} latency_ms {latency:.1} x {:.0} y {:.0} w {:.0} h {:.0} scale {scale}",
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        ));
        // The harness screenshots the window during this idle stretch.
        sleep(cx, 1500).await;
    }

    // 2. Nothing redraws while idle.
    let before = renders.get();
    sleep(cx, 1000).await;
    if renders.get() != before {
        fail(format!("{} renders while idle", renders.get() - before));
    }
    say("IDLE OK".into());

    // 3. An animation started from an idle window runs at the display rate.
    let before = renders.get();
    this.update(cx, |this, cx| {
        this.animate_until = Some(Instant::now() + Duration::from_millis(1000));
        cx.notify();
    })
    .unwrap_or_else(|_| fail("window gone".into()));
    sleep(cx, 1300).await;
    let frames = renders.get() - before;
    say(format!("ANIMATION frames {frames}"));
    // CI renders in software, so the rate is far below the display's; a
    // window left with single frames would draw one or two.
    if frames < 10 {
        fail(format!("animation drew {frames} frames in 1 s"));
    }
    sleep(cx, 1000).await;

    // 4. A next-frame callback on an idle window runs.
    let ran = Rc::new(Cell::new(false));
    cx.update({
        let ran = ran.clone();
        move |window, _| window.on_next_frame(move |_, _| ran.set(true))
    })
    .unwrap_or_else(|_| fail("window gone".into()));
    let latency = wait_for(cx, "on_next_frame", 1000, || ran.get()).await;
    say(format!("NEXT_FRAME latency_ms {latency:.1}"));
    sleep(cx, 1000).await;

    // 5. Resizing an idle window draws it at the new size.
    let target = size(px(520.), px(360.));
    cx.update(|window, _| window.resize(target))
        .unwrap_or_else(|_| fail("window gone".into()));
    let latency = wait_for(cx, "resize", 2000, || {
        let viewport = viewport.get();
        (f32::from(viewport.width) - 520.).abs() < 2. && (f32::from(viewport.height) - 360.).abs() < 2.
    })
    .await;
    say(format!("RESIZE latency_ms {latency:.1}"));

    say("CHECK OK".into());
    std::process::exit(0);
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "idle".into());
    application().run(move |cx: &mut App| {
        let bounds = Bounds {
            origin: point(px(200.), px(200.)),
            size: size(px(480.), px(320.)),
        };
        let renders = Rc::new(Cell::new(0));
        let viewport = Rc::new(Cell::new(Size::default()));
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            {
                let renders = renders.clone();
                let viewport = viewport.clone();
                move |window, cx| {
                    cx.new(|cx| {
                        match mode.as_str() {
                            "check" => {
                                let renders = renders.clone();
                                let viewport = viewport.clone();
                                cx.spawn_in(window, async move |this, cx| {
                                    check(this, renders, viewport, cx).await
                                })
                                .detach();
                            }
                            "clock" => {
                                cx.spawn_in(window, async move |this, cx| {
                                    let mut tick = 0usize;
                                    loop {
                                        sleep(cx, 1000).await;
                                        tick += 1;
                                        if this
                                            .update(cx, |this, cx| {
                                                this.color = COLORS[tick % COLORS.len()];
                                                cx.notify();
                                            })
                                            .is_err()
                                        {
                                            return;
                                        }
                                    }
                                })
                                .detach();
                            }
                            _ => {}
                        }
                        Probe {
                            color: COLORS[0],
                            renders,
                            viewport,
                            animate_until: None,
                        }
                    })
                }
            },
        )
        .unwrap();
        cx.activate(true);
        say("READY".into());
    });
}
