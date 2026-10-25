pub mod native_review {
    use super::*;
    use gpui::{AppContext, Render, ScrollHandle, VisualTestAppContext};
    use serde_json::json;
    struct Harness {
        input: Entity<TextInputState>, scroll: ScrollHandle,
        line_height: f32, font_size: f32, align: TextAlign, family: SharedString, color: Hsla,
    }
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(gpui::rgb(0x151515)).font_family(self.family.clone())
                .child(div().id("review-scroll").track_scroll(&self.scroll).size_full().overflow_y_scroll()
                    .child(div().w_full().px(px(32.)).child(TextInput::bare(&Theme::dark(), "review-input", &self.input)
                        .text_size(px(self.font_size)).line_height(px(self.line_height)).align(self.align).text_color(self.color))))
        }
    }
    pub fn run() {
        let output=std::path::PathBuf::from(std::env::var("CAP_NATIVE_OUTPUT").unwrap());
        std::fs::create_dir_all(&output).unwrap();
        let mut cx=VisualTestAppContext::new(gpui_platform::current_platform(true));
        cx.update(bind_keys);
        let win=cx.open_offscreen_window(size(px(560.), px(320.)), |window,cx| cx.new(|cx| {
            Harness { input: cx.new(|cx| TextInputState::multi_line(window,cx)), scroll: ScrollHandle::new(),
                line_height: 45., font_size: 30., align: TextAlign::Center, family: ".SystemUIFont".into(), color: gpui::white() }
        })).unwrap();
        let root=win.root(&mut cx).unwrap();
        let any: gpui::AnyWindowHandle=win.into();
        let mut records=Vec::new(); let mut id=0;
        for (kind, unit) in [
            ("ascii", "Welcome to the recording. This is a paragraph for wrapping and clipping.\n"),
            ("unicode", "café e\u{301} 中文 हिन्दी مرحبا 👩🏽‍💻 🇬🇧\n"),
            ("empty-lines", "\n\nShort\n\n"),
            ("single-paragraph", "The recording should look exactly the same as before. "),
        ] {
            for line_height in [16.25, 26.4, 45.3] {
                let text=unit.repeat(150);
                cx.update(|cx| root.update(cx, |v,cx| {
                    v.line_height=line_height;
                    v.input.update(cx,|s,cx| s.set_text(text.clone(),cx)); cx.notify();
                })); cx.run_until_parked();
                for position in [0., 0.5, 1.] {
                    cx.update_window(any, |_,w,cx| {
                        root.update(cx,|v,cx| {
                            v.scroll.set_offset(point(px(0.), -v.scroll.max_offset().y * position));
                            v.input.update(cx,|s,cx| s.move_to(0,cx)); cx.notify();
                        }); w.refresh();
                    }).unwrap(); cx.run_until_parked();
                    for mode in ["plain", "selected", "composition"] {
                        cx.update_window(any, |_,w,cx| {
                            let input=root.read(cx).input.clone();
                            input.update(cx,|s,cx| {
                                if mode=="selected" { s.select_all_text(cx); }
                                else { s.move_to(0,cx); }
                                s.marked_range=if mode=="composition" { Some(0..text.chars().next().unwrap().len_utf8()) } else { None };
                            });
                            w.refresh();
                        }).unwrap(); cx.run_until_parked();
                        let image=cx.capture_screenshot(any).unwrap();
                        assert!(image.pixels().filter(|p| p[0]>150 && p[1]>150 && p[2]>150 && p[3]>0).count()>20,
                            "native capture {id} has no visible text");
                        image.save(output.join(format!("{id:03}.png"))).unwrap();
                        let geometry=cx.update(|cx| root.read(cx).input.read(cx).text_bounds);
                        records.push(json!({"id":id,"kind":kind,"line_height":line_height,"position":position,"mode":mode,
                            "image_size":[image.width(),image.height()],"bounds":format!("{geometry:?}")}));
                        id+=1;
                    }
                }
            }
        }
        for (family,color) in [("Arial",0xff5555), ("Courier",0x55ff55), ("Times New Roman",0x5555ff), (".SystemUIFont",0xffffff)] {
            cx.update_window(any,|_,w,cx| {
                root.update(cx,|v,cx| {
                    v.family=family.into();
                    v.color=gpui::rgb(color).into();
                    v.input.update(cx,|s,_| s.marked_range=None);
                    v.scroll.set_offset(point(px(0.),px(0.))); cx.notify();
                }); w.refresh();
            }).unwrap(); cx.run_until_parked();
            let warm=cx.capture_screenshot(any).unwrap();
            cx.update_window(any,|_,w,cx| {
                let input=root.read(cx).input.clone();
                input.update(cx,|s,_| s.invalidate_layout()); w.refresh();
            }).unwrap(); cx.run_until_parked();
            let fresh=cx.capture_screenshot(any).unwrap();
            assert_eq!(warm.as_raw(),fresh.as_raw(),"cached font {family} differs from a fresh layout");
        }
        std::fs::write(output.join("manifest.json"),serde_json::to_vec_pretty(&records).unwrap()).unwrap();
        cx.update_window(any, |_,w,_| w.remove_window()).unwrap();
        println!("Captured {id} native Metal/font cases");
    }
}
