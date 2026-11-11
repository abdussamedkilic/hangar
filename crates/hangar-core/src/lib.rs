pub mod artwork;
pub mod db;
pub mod library;
pub mod local_scan;
pub mod merge;
pub mod model;
pub mod providers;
pub mod vdf;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_set() {
        assert!(!version().is_empty());
    }
}
