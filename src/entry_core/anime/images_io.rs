//! The file side of the image actions of an anime (T-39a-046, T-39a-058): read the picture of a page, save it to the
//! pictures folder, stage it for sharing, copy a picked file in as the custom image, and delete the custom image. Every
//! name and every failure goes through the rules of `images.rs`: the extension from the bytes, `cover.<ext>` or
//! `background.<ext>`, a safe folder name (D-4), no image is an error (D-2), a failed open and an empty share are errors
//! (D-13, D-15). No GTK here; the window is `images_view`.

use std::path::{Path, PathBuf};

use kuro_images::art::{ArtKind, ArtRef, Owner};
use kuro_images::cover_file::write_atomic;
use kuro_images::fetch::{ImageService, Payload};
use kuro_images::options::LoadOptions;
use kuro_data::anime::Anime;
use tokio_util::sync::CancellationToken;

use super::images::{ImageError, ImageKind, image_extension, sanitize_dir_name, saved_file_name};

/// Where the files of one image live: its custom file, the folder a save goes to (`<pictures>/Kurobayomi`), and the
/// folder a share stages its file in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Targets {
    pub custom: PathBuf,
    pub pictures: PathBuf,
    pub staging: PathBuf,
}

/// The image of `kind` of an anime as the image service names it. D-3: a background uses the background url.
pub fn art_of(anime: &Anime, kind: ImageKind) -> ArtRef {
    let (art_kind, last_modified) = match kind {
        ImageKind::Cover => (ArtKind::Cover, anime.cover_last_modified),
        ImageKind::Background => (ArtKind::Background, anime.background_last_modified),
    };
    ArtRef {
        owner: Owner::Anime,
        entry_id: anime.id,
        source_id: anime.source,
        kind: art_kind,
        url: kind.source_url(anime).map(str::to_owned),
        favorite: anime.favorite,
        last_modified,
    }
}

/// The bytes of the picture the window shows: the custom file when there is one, else what the service fetches (its disk
/// cache first). No url, no service and a failed fetch all end in `NoImage` (D-2); an unreadable custom file is `OpenFailed`.
pub async fn current_bytes(service: Option<&ImageService>, art: &ArtRef, custom: &Path) -> Result<Vec<u8>, ImageError> {
    if tokio::fs::try_exists(custom).await.unwrap_or(false) {
        return tokio::fs::read(custom).await.map_err(|_| ImageError::OpenFailed);
    }
    if art.url.is_none() {
        return Err(ImageError::NoImage);
    }
    let service = service.ok_or(ImageError::NoImage)?;
    let fetched = service.fetch(art, &LoadOptions { use_custom_cover: false, ..LoadOptions::default() }, &CancellationToken::new()).await.map_err(|_| ImageError::NoImage)?;
    match fetched.payload {
        Payload::File(path) => tokio::fs::read(path).await.map_err(|_| ImageError::OpenFailed),
        Payload::Bytes(bytes) => Ok(bytes.to_vec()),
    }
}

/// Save: writes `<pictures>/<safe title>/cover.<ext>` or `background.<ext>`, replacing a file of that name. Bytes that are
/// not a PNG, JPEG or WebP picture are not an image.
pub async fn save(bytes: &[u8], kind: ImageKind, title: &str, targets: &Targets) -> Result<PathBuf, ImageError> {
    let ext = image_extension(bytes).ok_or(ImageError::NoImage)?;
    let target = targets.pictures.join(sanitize_dir_name(title)).join(saved_file_name(kind, ext));
    write_atomic(&target, bytes).await.map_err(|_| ImageError::Io)?;
    Ok(target)
}

/// Share: writes the picture to the staging folder, which is emptied first so only the latest share is kept, and returns
/// the file for the share action. No bytes is the share error of D-15, not a silent success.
pub async fn stage_for_share(bytes: &[u8], kind: ImageKind, targets: &Targets) -> Result<PathBuf, ImageError> {
    if bytes.is_empty() {
        return Err(ImageError::EmptyShare);
    }
    let ext = image_extension(bytes).ok_or(ImageError::NoImage)?;
    match tokio::fs::remove_dir_all(&targets.staging).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(ImageError::Io),
    }
    let target = targets.staging.join(saved_file_name(kind, ext));
    write_atomic(&target, bytes).await.map_err(|_| ImageError::Io)?;
    Ok(target)
}

/// Edit: copies the picked file over the custom image. The bytes are not checked (behaviour 24.3); a file that cannot be
/// opened is `OpenFailed` (D-13), and nothing is changed then.
pub async fn set_custom(picked: &Path, targets: &Targets) -> Result<(), ImageError> {
    let bytes = tokio::fs::read(picked).await.map_err(|_| ImageError::OpenFailed)?;
    write_atomic(&targets.custom, &bytes).await.map_err(|_| ImageError::Io)
}

/// Delete: removes the custom image. A file that is not there is already deleted.
pub async fn delete_custom(targets: &Targets) -> Result<(), ImageError> {
    match tokio::fs::remove_file(&targets.custom).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ImageError::Io),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 9];

    fn run<T>(f: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(f)
    }

    fn targets(root: &Path) -> Targets {
        Targets { custom: root.join("data/custom/abc"), pictures: root.join("Pictures/Kurobayomi"), staging: root.join("cache/shared_image") }
    }

    #[test]
    fn save_writes_the_cover_under_a_safe_folder_with_the_extension_of_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let t = targets(dir.path());
        let path = run(save(PNG, ImageKind::Cover, "Fate/Zero: Part 1", &t)).unwrap();
        assert_eq!(path, dir.path().join("Pictures/Kurobayomi/Fate_Zero_ Part 1/cover.png"));
        assert_eq!(std::fs::read(&path).unwrap(), PNG);
        let path = run(save(JPEG, ImageKind::Background, "Fate/Zero: Part 1", &t)).unwrap();
        assert!(path.ends_with("background.jpg"));
        run(save(JPEG, ImageKind::Cover, "x", &t)).unwrap();
        let again = run(save(PNG, ImageKind::Cover, "x", &t)).unwrap();
        assert!(again.ends_with("cover.png"), "a second save with another type is another file");
    }

    #[test]
    fn saving_something_that_is_not_a_picture_is_the_no_image_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(run(save(b"plain text", ImageKind::Cover, "x", &targets(dir.path()))), Err(ImageError::NoImage));
        assert!(!dir.path().join("Pictures").exists(), "nothing was written");
    }

    #[test]
    fn a_save_that_cannot_write_is_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("Pictures");
        std::fs::write(&blocker, b"a file, not a folder").unwrap();
        assert_eq!(run(save(PNG, ImageKind::Cover, "x", &targets(dir.path()))), Err(ImageError::Io));
    }

    #[test]
    fn sharing_stages_one_file_and_replaces_the_one_before() {
        let dir = tempfile::tempdir().unwrap();
        let t = targets(dir.path());
        let first = run(stage_for_share(PNG, ImageKind::Cover, &t)).unwrap();
        assert!(first.is_file());
        let second = run(stage_for_share(JPEG, ImageKind::Background, &t)).unwrap();
        assert!(second.is_file() && !first.exists(), "only the latest share is kept");
        assert_eq!(std::fs::read_dir(&t.staging).unwrap().count(), 1);
    }

    #[test]
    fn an_empty_share_is_the_share_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(run(stage_for_share(&[], ImageKind::Cover, &targets(dir.path()))), Err(ImageError::EmptyShare));
    }

    #[test]
    fn edit_copies_the_picked_file_and_delete_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let t = targets(dir.path());
        let picked = dir.path().join("mine.png");
        std::fs::write(&picked, PNG).unwrap();
        run(set_custom(&picked, &t)).unwrap();
        assert_eq!(std::fs::read(&t.custom).unwrap(), PNG);
        run(delete_custom(&t)).unwrap();
        assert!(!t.custom.exists());
        run(delete_custom(&t)).unwrap();
    }

    #[test]
    fn an_edit_with_a_file_that_cannot_be_opened_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let t = targets(dir.path());
        std::fs::create_dir_all(t.custom.parent().unwrap()).unwrap();
        std::fs::write(&t.custom, JPEG).unwrap();
        assert_eq!(run(set_custom(&dir.path().join("missing.png"), &t)), Err(ImageError::OpenFailed));
        assert_eq!(std::fs::read(&t.custom).unwrap(), JPEG, "D-13: the old image stays");
    }

    #[test]
    fn the_picture_is_the_custom_file_first_and_no_url_is_no_image() {
        let dir = tempfile::tempdir().unwrap();
        let t = targets(dir.path());
        let anime = Anime { id: 3, thumbnail_url: None, background_url: Some("https://a/b.jpg".into()), ..Anime::create() };
        let cover = art_of(&anime, ImageKind::Cover);
        assert_eq!(cover.url, None);
        assert_eq!(run(current_bytes(None, &cover, &t.custom)), Err(ImageError::NoImage), "D-2");
        std::fs::create_dir_all(t.custom.parent().unwrap()).unwrap();
        std::fs::write(&t.custom, PNG).unwrap();
        assert_eq!(run(current_bytes(None, &cover, &t.custom)).unwrap(), PNG);
        let back = art_of(&anime, ImageKind::Background);
        assert_eq!((back.kind, back.url.as_deref()), (ArtKind::Background, Some("https://a/b.jpg")), "D-3");
        std::fs::remove_file(&t.custom).unwrap();
        assert_eq!(run(current_bytes(None, &back, &t.custom)), Err(ImageError::NoImage), "a url but no image service");
    }
}
