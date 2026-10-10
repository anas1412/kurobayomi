//! Decoder page rows (tasks T-48a-026 to T-48a-028): hardware decoding, gpu-next, debanding and the YUV420P pixel
//! format. mpv reads these when a video starts, so the page says that a change applies to the next video. The mpv
//! options themselves come from `kuro_prefs::player::mpv_decoder_options`.
use kuro_prefs::player::Debanding;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::rows::{enum_choice, info, switch, switch_sub};

pub const GPU_NEXT_SUBTITLE: &str = "A new video rendering backend";
pub const YUV420P_SUBTITLE: &str = "May fix black screens on some video codecs, can also improve performance at the cost of quality";
pub const NEXT_VIDEO_NOTE: &str = "Changes apply the next time a video starts.";

/// The debanding options in list order, labelled with the raw names of the Kotlin app.
pub const DEBANDING: [(Debanding, &str); 3] = [(Debanding::None, "None"), (Debanding::CPU, "CPU"), (Debanding::GPU, "GPU")];

/// The rows of the Decoder page, loose (the spec page has no group title).
pub fn page_rows(prefs: &Prefs) -> Vec<PreferenceItem> {
    let d = &prefs.decoder;
    vec![
        switch("Try hardware decoding", &d.try_hw_decoding, true),
        switch_sub("Enable gpu-next", GPU_NEXT_SUBTITLE, &d.gpu_next, true),
        enum_choice("Debanding", &d.video_debanding, &DEBANDING, true),
        switch_sub("Use YUV420P pixel format", YUV420P_SUBTITLE, &d.use_yuv420p, true),
        info(NEXT_VIDEO_NOTE),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_hardware_on_gpu_next_off_no_debanding_and_yuv420p_on() {
        let prefs = Prefs::in_memory();
        assert!(prefs.decoder.try_hw_decoding.get());
        assert!(!prefs.decoder.gpu_next.get());
        assert_eq!(prefs.decoder.video_debanding.get(), Debanding::None);
        assert!(prefs.decoder.use_yuv420p.get());
    }

    #[test]
    fn the_page_lists_the_four_rows_then_the_note_in_spec_order() {
        let titles: Vec<String> = page_rows(&Prefs::in_memory()).iter().map(|i| i.title().to_owned()).collect();
        assert_eq!(
            titles,
            ["Try hardware decoding", "Enable gpu-next", "Debanding", "Use YUV420P pixel format", NEXT_VIDEO_NOTE]
        );
    }

    #[test]
    fn debanding_offers_none_cpu_and_gpu_with_the_stored_one_selected() {
        let prefs = Prefs::in_memory();
        prefs.decoder.video_debanding.set(Debanding::GPU).unwrap();
        let rows = page_rows(&prefs);
        let PreferenceItem::BasicList { value, entries, .. } = &rows[2] else { panic!("a list") };
        assert_eq!(value, "GPU");
        let labels: Vec<&str> = entries.iter().map(|(_, label)| label.as_str()).collect();
        assert_eq!(labels, ["None", "CPU", "GPU"]);
    }

    #[test]
    fn the_yuv_row_carries_its_two_line_subtitle() {
        let rows = page_rows(&Prefs::in_memory());
        let PreferenceItem::Switch { common, .. } = &rows[3] else { panic!("a switch") };
        assert_eq!(common.subtitle.as_deref(), Some(YUV420P_SUBTITLE));
    }
}
