//! Two live queries joined into one (`GetMangaWithChapters.subscribe`, `GetAnimeWithEpisodesAndSeasons`).
//! The joined value is published only after both sources have a published state, and again after every change of
//! either. A source that failed makes the joined value the error text (the first failed source in argument order).
//! Dropping the returned receiver stops the task.

use std::sync::Arc;

use kuro_db::subscribe::Subscription;
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// A pair of values, each shared the way a subscription shares its result.
pub type Pair<A, B> = (Arc<A>, Arc<B>);

/// A triple of values, each shared the way a subscription shares its result.
pub type Triple<A, B, C> = (Arc<A>, Arc<B>, Arc<C>);

/// The receiver of a joined subscription and the task behind it.
pub type Joined<T> = (watch::Receiver<Option<T>>, JoinHandle<()>);

/// Joins two subscriptions. The returned handle finishes once the receiver is dropped or either source
/// stops (its database handle went away).
pub fn combine2<A, B>(
    mut a: Subscription<A>,
    mut b: Subscription<B>,
) -> Joined<Result<Pair<A, B>, String>>
where
    A: Send + Sync + 'static,
    B: Send + Sync + 'static,
{
    let (tx, rx) = watch::channel(None);
    let handle = tokio::spawn(async move {
        let mut left = a.current();
        let mut right = b.current();
        loop {
            if let (Some(l), Some(r)) = (&left, &right) {
                let joined = match (l, r) {
                    (Ok(l), Ok(r)) => Ok((Arc::clone(l), Arc::clone(r))),
                    (Err(e), _) | (_, Err(e)) => Err(e.clone()),
                };
                if tx.send(Some(joined)).is_err() {
                    break;
                }
            }
            tokio::select! {
                _ = tx.closed() => break,
                next = a.changed() => match next {
                    Some(v) => left = Some(v),
                    None => break,
                },
                next = b.changed() => match next {
                    Some(v) => right = Some(v),
                    None => break,
                },
            }
        }
    });
    (rx, handle)
}

/// Joins three subscriptions, with the same rules as [`combine2`]: published after every source has a value,
/// then on every change of any of them.
pub fn combine3<A, B, C>(
    mut a: Subscription<A>,
    mut b: Subscription<B>,
    mut c: Subscription<C>,
) -> Joined<Result<Triple<A, B, C>, String>>
where
    A: Send + Sync + 'static,
    B: Send + Sync + 'static,
    C: Send + Sync + 'static,
{
    let (tx, rx) = watch::channel(None);
    let handle = tokio::spawn(async move {
        let mut first = a.current();
        let mut second = b.current();
        let mut third = c.current();
        loop {
            if let (Some(x), Some(y), Some(z)) = (&first, &second, &third) {
                let joined = match (x, y, z) {
                    (Ok(x), Ok(y), Ok(z)) => Ok((Arc::clone(x), Arc::clone(y), Arc::clone(z))),
                    (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e.clone()),
                };
                if tx.send(Some(joined)).is_err() {
                    break;
                }
            }
            tokio::select! {
                _ = tx.closed() => break,
                next = a.changed() => match next {
                    Some(v) => first = Some(v),
                    None => break,
                },
                next = b.changed() => match next {
                    Some(v) => second = Some(v),
                    None => break,
                },
                next = c.changed() => match next {
                    Some(v) => third = Some(v),
                    None => break,
                },
            }
        }
    });
    (rx, handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::manga_repo;
    use kuro_data::manga::{Manga, MangaUpdate};
    use std::time::Duration;

    /// Waits until a published pair satisfies `done`. Intermediate pairs (one source updated, the other not
    /// yet re-queried) are skipped.
    async fn wait_for<A, B>(
        rx: &mut watch::Receiver<Option<Result<Pair<A, B>, String>>>,
        done: impl Fn(&Pair<A, B>) -> bool,
    ) -> Pair<A, B>
    where
        A: Send + Sync + 'static,
        B: Send + Sync + 'static,
    {
        loop {
            if let Some(pair) = rx.borrow_and_update().clone() {
                let pair = pair.expect("the joined pair is not an error");
                if done(&pair) {
                    return pair;
                }
            }
            tokio::time::timeout(Duration::from_secs(5), rx.changed())
                .await
                .expect("the expected pair within 5 s")
                .expect("sender alive");
        }
    }

    #[tokio::test]
    async fn emits_after_both_sources_then_on_every_change_of_either() {
        let (_dir, repo) = manga_repo();
        let id = repo
            .insert(&Manga {
                url: "/m".into(),
                title: "T".into(),
                source: 1,
                ..Manga::create()
            })
            .expect("insert");
        let manga_sub = repo.subscribe_by_id(id);
        let favs_sub = repo.subscribe_favorites_by_source(1);
        let (mut rx, _handle) = combine2(manga_sub, favs_sub);

        let (m, favs) = wait_for(&mut rx, |_| true).await;
        assert_eq!(
            m.as_ref().as_ref().map(|x| x.title.clone()),
            Some("T".into())
        );
        assert!(favs.is_empty());

        repo.update(&MangaUpdate {
            favorite: Some(true),
            ..MangaUpdate::new(id)
        })
        .expect("update");
        // Wait for the pair where both sources show the change: the favourites list can publish before the row does.
        let (m2, _favs2) = wait_for(&mut rx, |(m, f)| f.len() == 1 && m.as_ref().as_ref().is_some_and(|x| x.favorite)).await;
        assert_eq!(m2.as_ref().as_ref().map(|x| x.favorite), Some(true));

        repo.update(&MangaUpdate {
            title: Some("U".into()),
            ..MangaUpdate::new(id)
        })
        .expect("update");
        let (m3, _) = wait_for(&mut rx, |(m, _)| {
            m.as_ref().as_ref().map(|x| x.title.as_str()) == Some("U")
        })
        .await;
        assert_eq!(
            m3.as_ref().as_ref().map(|x| x.title.clone()),
            Some("U".into()),
            "change of the first source"
        );
    }

    #[tokio::test]
    async fn dropping_the_receiver_stops_the_task() {
        let (_dir, repo) = manga_repo();
        let id = repo
            .insert(&Manga {
                url: "/m".into(),
                title: "T".into(),
                source: 1,
                ..Manga::create()
            })
            .expect("insert");
        let (rx, handle) = combine2(
            repo.subscribe_by_id(id),
            repo.subscribe_favorites_by_source(1),
        );
        drop(rx);
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("task ends after the receiver is dropped")
            .expect("task did not panic");
    }

    #[tokio::test]
    async fn combine3_emits_after_all_three_then_on_every_change() {
        use crate::testdb::anime_repo;
        use kuro_data::anime::{Anime, AnimeUpdate};

        let (_dir, repo) = anime_repo();
        let id = repo
            .insert(&Anime {
                url: "/a".into(),
                title: "T".into(),
                source: 1,
                ..Anime::create()
            })
            .expect("insert");
        let (mut rx, _handle) = combine3(
            repo.subscribe_by_id(id),
            repo.subscribe_favorites_by_source(1),
            repo.subscribe_seasons_by_parent(id),
        );
        let (a, favs, seasons) = wait_for3(&mut rx, |_| true).await;
        assert_eq!(a.as_ref().as_ref().map(|x| x.title.clone()), Some("T".into()));
        assert!(favs.is_empty());
        assert!(seasons.is_empty());

        repo.update(&AnimeUpdate {
            favorite: Some(true),
            ..AnimeUpdate::new(id)
        })
        .expect("update");
        let (_, favs2, _) = wait_for3(&mut rx, |(_, f, _)| f.len() == 1).await;
        assert_eq!(favs2.len(), 1, "change of the second source");
    }

    /// Waits until a published triple satisfies `done`.
    async fn wait_for3<A, B, C>(
        rx: &mut watch::Receiver<Option<Result<Triple<A, B, C>, String>>>,
        done: impl Fn(&Triple<A, B, C>) -> bool,
    ) -> Triple<A, B, C>
    where
        A: Send + Sync + 'static,
        B: Send + Sync + 'static,
        C: Send + Sync + 'static,
    {
        loop {
            if let Some(triple) = rx.borrow_and_update().clone() {
                let triple = triple.expect("the joined triple is not an error");
                if done(&triple) {
                    return triple;
                }
            }
            tokio::time::timeout(Duration::from_secs(5), rx.changed())
                .await
                .expect("the expected triple within 5 s")
                .expect("sender alive");
        }
    }

    #[tokio::test]
    async fn dropping_the_receiver_stops_the_three_source_task() {
        use crate::testdb::anime_repo;
        use kuro_data::anime::Anime;

        let (_dir, repo) = anime_repo();
        let id = repo
            .insert(&Anime {
                url: "/a".into(),
                title: "T".into(),
                source: 1,
                ..Anime::create()
            })
            .expect("insert");
        let (rx, handle) = combine3(
            repo.subscribe_by_id(id),
            repo.subscribe_favorites_by_source(1),
            repo.subscribe_seasons_by_parent(id),
        );
        drop(rx);
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("task ends after the receiver is dropped")
            .expect("task did not panic");
    }
}
