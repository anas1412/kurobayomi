// Manga reader and anime player: full-window views with overlay controls.
use crate::ui::*;
use gtk::prelude::*;
use gtk::{glib, Align, Box as GBox, Button, Label, Orientation as O};
use std::time::Duration;

fn sheet(screen_id: &str, sheet_id: &str) {
    if let Some(s) = screen(screen_id).sheets.iter().find(|s| s.id == sheet_id) {
        show_sheet(s);
    }
}

fn dialog(screen_id: &str, dialog_id: &str) {
    if let Some(d) = screen(screen_id).dialogs.iter().find(|d| d.id == dialog_id) {
        show_dialog(d);
    }
}

fn osd(o: O, spacing: i32) -> GBox {
    let b = GBox::new(o, spacing);
    b.add_css_class("osd");
    b.add_css_class("toolbar");
    b
}

fn on_long_press(w: &impl IsA<gtk::Widget>, f: impl Fn() + 'static) {
    let g = gtk::GestureLongPress::new();
    g.connect_pressed(move |_, _, _| f());
    w.add_controller(g);
}

fn two_line(title: &str, sub: &str) -> GBox {
    let v = GBox::new(O::Vertical, 0);
    v.append(&Label::builder().label(title).xalign(0.0).css_classes(["heading"]).build());
    v.append(&Label::builder().label(sub).xalign(0.0).css_classes(["caption", "dim-label"]).build());
    v
}

/// Flash a short message centered over `overlay` (Aniyomi's toast / player update pill).
fn pill(overlay: &gtk::Overlay, text: &str) {
    let l = Label::builder().label(text).halign(Align::Center).valign(Align::Center).css_classes(["osd", "title-4"]).build();
    margins(&l, 0);
    l.set_margin_top(80);
    overlay.add_overlay(&l);
    let o = overlay.clone();
    glib::timeout_add_local_once(Duration::from_millis(1200), move || o.remove_overlay(&l));
}

fn fmt_time(s: f64) -> String {
    format!("{:02}:{:02}", s as u32 / 60, s as u32 % 60)
}

pub fn player(title: &str, episode: &str) -> GBox {
    let id = "player";
    // the custom-button scripts are written and checked at player start; the load commands wait for an mpv handle
    if let (true, Ok(paths)) = (crate::data::is_installed(), kuro_platform::linux::LinuxPaths::system()) {
        use kuro_platform::paths::Paths as _;
        crate::player_scripts::start(crate::data::app().anime_db.clone(), paths.data_dir(), |load| {
            tracing::debug!(scripts = load.len(), "custom button scripts are ready for mpv");
        });
    }
    let overlay = gtk::Overlay::new();
    let duration = 1420.0;

    // ponytail: placeholder surface; swap for an mpv/GStreamer widget (gtk::Video handles local files)
    let surface = GBox::new(O::Vertical, 0);
    surface.add_css_class("osd");
    surface.set_vexpand(true);
    overlay.set_child(Some(&surface));

    // top
    let top = GBox::new(O::Horizontal, 6);
    top.add_css_class("toolbar");
    top.set_valign(Align::Start);
    margins(&top, 12);
    let back = icon_button("go-previous-symbolic", "Back");
    back.connect_clicked(|_| pop());
    top.append(&back);
    let titles = Button::builder().child(&two_line(title, episode)).css_classes(["flat"]).build();
    titles.connect_clicked(|_| dialog("player", "episode_list"));
    top.append(&titles);
    let spacer = GBox::new(O::Horizontal, 0);
    spacer.set_hexpand(true);
    top.append(&spacer);
    let auto = gtk::Switch::builder().tooltip_text("Auto-play").valign(Align::Center).build();
    let o = overlay.clone();
    auto.connect_active_notify(move |s| pill(&o, if s.is_active() { "Auto-play is on" } else { "Auto-play is off" }));
    top.append(&auto);
    for (icon, tip, tap, long) in [
        ("media-view-subtitles-symbolic", "Subtitles", "subtitle_tracks", "panel_subtitle_settings"),
        ("audio-x-generic-symbolic", "Audio", "audio_tracks", "panel_audio_delay"),
        ("video-display-symbolic", "Qualities", "quality_tracks", "quality_tracks"),
        ("view-more-symbolic", "More", "more", "panel_video_filters"),
    ] {
        let b = icon_button(icon, tip);
        b.connect_clicked(move |_| sheet("player", tap));
        on_long_press(&b, move || sheet("player", long));
        top.append(&b);
    }
    overlay.add_overlay(&top);

    // middle
    let mid = GBox::new(O::Horizontal, 36);
    mid.add_css_class("toolbar");
    mid.set_halign(Align::Center);
    mid.set_valign(Align::Center);
    let big = |icon: &str, tip: &str| {
        let b = Button::builder().child(&gtk::Image::builder().icon_name(icon).pixel_size(32).build()).tooltip_text(tip).build();
        b.update_property(&[gtk::accessible::Property::Label(tip)]);
        b
    };
    let prev = big("media-skip-backward-symbolic", "Previous episode");
    let play = Button::builder().child(&gtk::Image::builder().icon_name("media-playback-pause-symbolic").pixel_size(48).build()).css_classes(["circular", "osd"]).build();
    play.update_property(&[gtk::accessible::Property::Label("Play or pause")]);
    play.connect_clicked(|b| {
        let img = b.child().and_downcast::<gtk::Image>().unwrap();
        let paused = img.icon_name().as_deref() == Some("media-playback-start-symbolic");
        img.set_icon_name(Some(if paused { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }));
    });
    let next = big("media-skip-forward-symbolic", "Next episode");
    for b in [&prev, &play, &next] {
        mid.append(b);
    }
    overlay.add_overlay(&mid);

    // bottom
    let bottom = GBox::new(O::Vertical, 6);
    bottom.set_valign(Align::End);
    margins(&bottom, 16);
    let buttons = GBox::new(O::Horizontal, 6);
    let lock = icon_button("changes-allow-symbolic", "Lock controls");
    let rotate = icon_button("object-rotate-right-symbolic", "Rotate");
    let speed = Button::with_label("1.00x");
    speed.connect_clicked(|b| {
        let v: f64 = b.label().unwrap().trim_end_matches('x').parse().unwrap_or(1.0);
        b.set_label(&format!("{:.2}x", if v >= 2.0 { 0.25 } else { v + 0.25 }));
    });
    on_long_press(&speed, || sheet("player", "playback_speed"));
    let chap = Button::with_label("00:00 • Opening");
    chap.connect_clicked(|_| sheet("player", "chapters"));
    for w in [lock.upcast_ref::<gtk::Widget>(), rotate.upcast_ref(), speed.upcast_ref(), chap.upcast_ref()] {
        buttons.append(w);
    }
    let sp = GBox::new(O::Horizontal, 0);
    sp.set_hexpand(true);
    buttons.append(&sp);
    let skip = Button::builder().label("Skip Opening").css_classes(["suggested-action", "pill"]).build();
    let aspect = icon_button("zoom-fit-best-symbolic", "Aspect ratio");
    let o = overlay.clone();
    let modes = ["Fit to screen", "Stretched to screen", "Cropped to screen"];
    let idx = std::rc::Rc::new(std::cell::Cell::new(0usize));
    aspect.connect_clicked(move |_| {
        idx.set((idx.get() + 1) % 3);
        pill(&o, modes[idx.get()]);
    });
    for w in [skip.upcast_ref::<gtk::Widget>(), aspect.upcast_ref()] {
        buttons.append(w);
    }
    bottom.append(&buttons);

    let seek = GBox::new(O::Horizontal, 8);
    let pos = Label::new(Some("00:00"));
    pos.set_width_chars(6);
    let bar = gtk::Scale::with_range(O::Horizontal, 0.0, duration, 1.0);
    bar.set_hexpand(true);
    bar.update_property(&[gtk::accessible::Property::Label("Seek")]);
    bar.add_mark(85.0, gtk::PositionType::Bottom, None);
    bar.add_mark(1330.0, gtk::PositionType::Bottom, None);
    let dur = Button::builder().label(fmt_time(duration)).css_classes(["flat"]).build();
    {
        let (pos, d2, bar2) = (pos.clone(), dur.clone(), bar.clone());
        bar.connect_value_changed(move |s| {
            pos.set_label(&fmt_time(s.value()));
            if d2.label().is_some_and(|l| l.starts_with('-')) {
                d2.set_label(&format!("-{}", fmt_time(duration - bar2.value())));
            }
        });
        let bar3 = bar.clone();
        dur.connect_clicked(move |d| {
            let remaining = d.label().is_some_and(|l| l.starts_with('-'));
            d.set_label(&if remaining { fmt_time(duration) } else { format!("-{}", fmt_time(duration - bar3.value())) });
        });
    }
    seek.append(&pos);
    seek.append(&bar);
    seek.append(&dur);
    bottom.append(&seek);
    let bg = osd(O::Vertical, 0);
    bg.set_valign(Align::End);
    bg.append(&bottom);
    overlay.add_overlay(&bg);

    // locked state: only an unlock button
    let unlock = icon_button("changes-prevent-symbolic", "Unlock");
    unlock.set_halign(Align::Start);
    unlock.set_valign(Align::Start);
    margins(&unlock, 12);
    unlock.set_visible(false);
    overlay.add_overlay(&unlock);
    let controls = [top.clone().upcast::<gtk::Widget>(), mid.clone().upcast(), bg.clone().upcast()];
    {
        let (c, u) = (controls.clone(), unlock.clone());
        lock.connect_clicked(move |_| {
            c.iter().for_each(|w| w.set_visible(false));
            u.set_visible(true);
        });
        let c = controls.clone();
        unlock.connect_clicked(move |u| {
            c.iter().for_each(|w| w.set_visible(true));
            u.set_visible(false);
        });
    }

    // gestures: single tap toggles controls, double tap seeks/pauses by fifths, long press = screenshot
    let click = gtk::GestureClick::new();
    {
        let (c, u, o, bar, s, play) = (controls.clone(), unlock.clone(), overlay.clone(), bar.clone(), surface.clone(), play.clone());
        click.connect_released(move |g, n, x, _| {
            if u.is_visible() {
                return;
            }
            let w = s.width() as f64;
            if n == 2 {
                if x < w * 0.4 {
                    bar.set_value(bar.value() - 10.0);
                    pill(&o, "-10 seconds");
                } else if x > w * 0.6 {
                    bar.set_value(bar.value() + 10.0);
                    pill(&o, "+10 seconds");
                } else {
                    play.emit_clicked();
                }
                g.set_state(gtk::EventSequenceState::Claimed);
            } else {
                let show = !c[0].is_visible();
                c.iter().for_each(|w| w.set_visible(show));
            }
        });
    }
    surface.add_controller(click);
    on_long_press(&surface, || sheet("player", "screenshot"));

    let b = GBox::new(O::Vertical, 0);
    overlay.set_vexpand(true);
    b.append(&overlay);
    tag(&b, id);
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-46a-054: keyboard focus follows the child order of the overlay, so the control bars must be added top, centre,
    /// bottom. The bars are told apart by their vertical alignment.
    #[test]
    fn the_control_bars_are_in_focus_order_top_centre_bottom() {
        crate::testing::with_display(|| {
            let root = player("Show", "Episode 1");
            let overlay = root.first_child().and_downcast::<gtk::Overlay>().expect("the overlay");
            let mut aligns = Vec::new();
            let mut child = overlay.first_child();
            while let Some(widget) = child {
                if widget.has_css_class("toolbar") && widget.valign() != Align::Fill {
                    aligns.push(widget.valign());
                }
                child = widget.next_sibling();
            }
            assert_eq!(aligns, [Align::Start, Align::Center, Align::End]);
        });
    }
}
