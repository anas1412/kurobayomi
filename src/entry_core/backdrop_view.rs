//! The backdrop behind the manga header (T-38a-014): the blurred cover at alpha 0.2 with a gradient from transparent to the
//! window background (`backdrop.rs`). The blur is made once per cover and kept in a cache keyed by the cover's url and
//! modification time, so a header drawn again shows it at once and a changed cover is blurred again.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gtk::prelude::*;
use gtk::{gdk, glib};
use kuro_images::art::ArtRef;

use super::anime::images_io;
use super::backdrop::{BACKDROP_ALPHA, FADE_CLASS, cache_key, gradient_css, prepare};
use super::cover_view;
use super::full_cover_view::offload;

thread_local! {
    static CACHE: RefCell<HashMap<String, gdk::Texture>> = RefCell::default();
    static CSS_DONE: Cell<bool> = const { Cell::new(false) };
}

fn ensure_css() {
    if CSS_DONE.with(|d| d.replace(true)) {
        return;
    }
    if let Some(display) = gdk::Display::default() {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&gradient_css());
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}

/// The blurred picture of a cover already made, if any.
fn cached(key: &str) -> Option<gdk::Texture> {
    CACHE.with(|c| c.borrow().get(key).cloned())
}

/// How many blurred covers are kept.
pub fn cached_count() -> usize {
    CACHE.with(|c| c.borrow().len())
}

/// The backdrop widget: it fills the space it is given and shows nothing until the picture is ready.
pub fn build(art: Option<&ArtRef>) -> gtk::Overlay {
    ensure_css();
    let picture = gtk::Picture::builder().content_fit(gtk::ContentFit::Cover).can_shrink(true).hexpand(true).vexpand(true).opacity(BACKDROP_ALPHA).build();
    let fade = gtk::Box::builder().css_classes([FADE_CLASS]).hexpand(true).vexpand(true).build();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&picture));
    overlay.add_overlay(&fade);
    overlay.set_can_target(false);
    if let Some(art) = art.filter(|a| a.url.is_some()) {
        load(art.clone(), &picture);
    }
    overlay
}

fn load(art: ArtRef, picture: &gtk::Picture) {
    let key = cache_key(art.url.as_deref().unwrap_or_default(), art.last_modified);
    if let Some(texture) = cached(&key) {
        picture.set_paintable(Some(&texture));
        return;
    }
    let (Some(service), Ok(runtime)) = (cover_view::service(), tokio::runtime::Handle::try_current()) else { return };
    let custom = kuro_platform::linux::LinuxPaths::system().ok().map(|paths| kuro_images::cover_paths::custom_file_for(&paths, &art)).unwrap_or_default();
    let rx = offload(&runtime, move || async move {
        let bytes = images_io::current_bytes(Some(service.as_ref()), &art, &custom).await.ok()?;
        prepare(&bytes)
    });
    let picture = picture.downgrade();
    glib::spawn_future_local(async move {
        let Ok(Some((w, h, rgba))) = rx.recv().await else { return };
        let bytes = glib::Bytes::from_owned(rgba);
        let texture: gdk::Texture = gdk::MemoryTexture::new(w as i32, h as i32, gdk::MemoryFormat::R8g8b8a8, &bytes, (w * 4) as usize).upcast();
        CACHE.with(|c| c.borrow_mut().insert(key, texture.clone()));
        if let Some(picture) = picture.upgrade() {
            picture.set_paintable(Some(&texture));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_backdrop_is_faint_and_a_cached_cover_shows_at_once() {
        crate::testing::with_display(|| {
            let bytes = glib::Bytes::from_owned(vec![1u8; 4 * 4 * 4]);
            let texture: gdk::Texture = gdk::MemoryTexture::new(4, 4, gdk::MemoryFormat::R8g8b8a8, &bytes, 16).upcast();
            CACHE.with(|c| c.borrow_mut().insert(cache_key("https://x/c.jpg", 7), texture));
            let art = ArtRef {
                owner: kuro_images::art::Owner::Manga,
                entry_id: 1,
                source_id: 1,
                kind: kuro_images::art::ArtKind::Cover,
                url: Some("https://x/c.jpg".into()),
                favorite: false,
                last_modified: 7,
            };
            let overlay = build(Some(&art));
            let picture = overlay.child().and_downcast::<gtk::Picture>().expect("the cover picture");
            assert!((picture.opacity() - BACKDROP_ALPHA).abs() < 0.01);
            assert!(picture.paintable().is_some(), "the cached blur is used without a load");
            assert!(cached_count() >= 1);
            let bare = build(None).child().and_downcast::<gtk::Picture>().expect("picture");
            assert!(bare.paintable().is_none(), "no cover, no backdrop");
        });
    }
}
