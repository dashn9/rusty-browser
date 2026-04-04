use rand::seq::SliceRandom;
use rustenium_identity::IdentityCountryGeo;
use rustmani_common::config::ProxyList;

pub struct ProxySelector(ProxyList);

impl ProxySelector {
    pub fn from_file(path: &str) -> Option<Self> {
        ProxyList::load(path).map(Self)
    }

    /// Returns a random proxy for `geo`, falling back to a random proxy from
    /// all available geos when none match.
    pub fn select(&self, geo: &IdentityCountryGeo) -> Option<String> {
        let mut rng = rand::thread_rng();
        let by_geo = self.0.get_proxies_for_geo(Some(geo.as_str()));
        if !by_geo.is_empty() {
            return by_geo.choose(&mut rng).cloned();
        }
        let all = self.0.get_all();
        all.choose(&mut rng).map(|s| s.to_string())
    }
}
