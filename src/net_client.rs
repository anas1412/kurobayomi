//! The app's HTTP client. This build has no embedded WebKit view, so a Cloudflare challenge goes to the browser host:
//! the system browser opens the page and the cookies the user exports clear it (`NetworkClient::for_desktop`).

use std::sync::Arc;

use kuro_net::{NetError, NetworkClient};
use kuro_platform::opener::XdgOpener;
use kuro_platform::paths::Paths;
use kuro_prefs::network::NetworkPrefs;

/// The client over `paths`, with the browser host as the challenge solver.
pub fn build(prefs: &NetworkPrefs, paths: &dyn Paths) -> Result<NetworkClient, NetError> {
    let (toaster, _feed) = kuro_platform::toast::toast_channel();
    NetworkClient::for_desktop(prefs, paths, None, Arc::new(XdgOpener), toaster)
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;
    use kuro_testkit::TempPaths;

    use super::*;

    #[test]
    fn the_app_client_builds_without_a_web_view() {
        let paths = TempPaths::new().unwrap();
        assert!(build(&NetworkPrefs::new(PrefStore::in_memory()), &paths).is_ok());
    }
}
