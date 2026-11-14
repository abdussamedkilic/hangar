use super::{AuthError, StoredCred, UbiCred};
use crate::model::Platform;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub trait SecretStore: Send + Sync {
    /// `Err(Corrupt)` if the stored data cannot be decrypted.
    fn load(&self, p: Platform) -> Result<Option<Vec<u8>>, AuthError>;
    fn save(&self, p: Platform, data: &[u8]) -> std::io::Result<()>;
    /// Ok even when nothing is stored.
    fn delete(&self, p: Platform) -> std::io::Result<()>;
}

impl<T: SecretStore> SecretStore for Arc<T> {
    fn load(&self, p: Platform) -> Result<Option<Vec<u8>>, AuthError> {
        (**self).load(p)
    }
    fn save(&self, p: Platform, data: &[u8]) -> std::io::Result<()> {
        (**self).save(p, data)
    }
    fn delete(&self, p: Platform) -> std::io::Result<()> {
        (**self).delete(p)
    }
}

#[derive(Default)]
pub struct MemoryStore {
    inner: Mutex<HashMap<Platform, Vec<u8>>>,
    pub fail_save: AtomicBool,
    pub corrupt: AtomicBool,
    saves: AtomicUsize,
}

impl MemoryStore {
    pub fn saves(&self) -> usize {
        self.saves.load(Ordering::SeqCst)
    }
    pub fn raw(&self, p: Platform) -> Option<Vec<u8>> {
        self.inner.lock().unwrap().get(&p).cloned()
    }
}

impl SecretStore for MemoryStore {
    fn load(&self, p: Platform) -> Result<Option<Vec<u8>>, AuthError> {
        if self.corrupt.load(Ordering::SeqCst) {
            return Err(AuthError::Corrupt);
        }
        Ok(self.raw(p))
    }
    fn save(&self, p: Platform, data: &[u8]) -> std::io::Result<()> {
        if self.fail_save.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("save failed"));
        }
        self.saves.fetch_add(1, Ordering::SeqCst);
        self.inner.lock().unwrap().insert(p, data.to_vec());
        Ok(())
    }
    fn delete(&self, p: Platform) -> std::io::Result<()> {
        self.inner.lock().unwrap().remove(&p);
        Ok(())
    }
}

pub fn save_cred(store: &dyn SecretStore, c: &StoredCred) -> std::io::Result<()> {
    let mut json = serde_json::to_vec(c).map_err(std::io::Error::other)?;
    let r = store.save(Platform::Epic, &json);
    zeroize::Zeroize::zeroize(&mut json);
    r
}

pub fn load_cred(store: &dyn SecretStore) -> Result<Option<StoredCred>, AuthError> {
    let Some(mut raw) = store.load(Platform::Epic)? else {
        return Ok(None);
    };
    let parsed = serde_json::from_slice::<StoredCred>(&raw);
    zeroize::Zeroize::zeroize(&mut raw);
    match parsed {
        Ok(c) if c.v == 1 => Ok(Some(c)),
        // Readable but from an unknown format version: not usable, but not ours to delete.
        Ok(_) => Err(AuthError::NotConnected),
        Err(_) => Err(AuthError::Corrupt),
    }
}

pub fn save_ubi_cred(store: &dyn SecretStore, c: &UbiCred) -> std::io::Result<()> {
    let mut json = serde_json::to_vec(c).map_err(std::io::Error::other)?;
    let r = store.save(Platform::Ubisoft, &json);
    zeroize::Zeroize::zeroize(&mut json);
    r
}

pub fn load_ubi_cred(store: &dyn SecretStore) -> Result<Option<UbiCred>, AuthError> {
    let Some(mut raw) = store.load(Platform::Ubisoft)? else {
        return Ok(None);
    };
    let parsed = serde_json::from_slice::<UbiCred>(&raw);
    zeroize::Zeroize::zeroize(&mut raw);
    match parsed {
        Ok(c) if c.v == 2 => Ok(Some(c)),
        Ok(_) => Err(AuthError::NotConnected),
        Err(_) => Err(AuthError::Corrupt),
    }
}

#[cfg(windows)]
pub use win::DpapiStore;

#[cfg(windows)]
mod win {
    use super::SecretStore;
    use crate::accounts::AuthError;
    use crate::model::Platform;
    use std::path::PathBuf;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    const ENTROPY: &[u8] = b"hangar-accounts-v1";

    pub struct DpapiStore {
        dir: PathBuf,
    }

    impl DpapiStore {
        pub fn new(dir: PathBuf) -> Self {
            Self { dir }
        }
        fn path(&self, p: Platform) -> PathBuf {
            self.dir.join(format!("{}.dat", p.as_str()))
        }
    }

    fn blob(d: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: d.len() as u32, pbData: d.as_ptr() as *mut u8 }
    }

    /// Copies the OS-allocated output into a Vec, wipes it when `wipe`, and frees it.
    unsafe fn take_out(out: CRYPT_INTEGER_BLOB, wipe: bool) -> Vec<u8> {
        if out.pbData.is_null() {
            return Vec::new();
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        if wipe {
            std::ptr::write_bytes(out.pbData, 0, out.cbData as usize);
        }
        LocalFree(out.pbData as *mut core::ffi::c_void);
        v
    }

    fn protect(data: &[u8]) -> std::io::Result<Vec<u8>> {
        if data.len() > u32::MAX as usize {
            return Err(std::io::Error::other("data too large"));
        }
        let input = blob(data);
        let entropy = blob(ENTROPY);
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY: input/entropy blobs point at live slices for the call; out is written by the OS.
        let ok = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: success => out holds a LocalAlloc'd buffer of cbData bytes.
        Ok(unsafe { take_out(out, false) })
    }

    fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
        if data.len() > u32::MAX as usize {
            return None;
        }
        let input = blob(data);
        let entropy = blob(ENTROPY);
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY: as in `protect`; description out-param is null (not requested).
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        if ok == 0 {
            return None;
        }
        // SAFETY: success => out is a LocalAlloc'd buffer; it holds plaintext, so wipe before freeing.
        Some(unsafe { take_out(out, true) })
    }

    impl SecretStore for DpapiStore {
        fn load(&self, p: Platform) -> Result<Option<Vec<u8>>, AuthError> {
            let raw = match std::fs::read(self.path(p)) {
                Ok(r) => r,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(_) => return Err(AuthError::Network("belge dosyası okunamadı".into())),
            };
            unprotect(&raw).map(Some).ok_or(AuthError::Corrupt)
        }
        fn save(&self, p: Platform, data: &[u8]) -> std::io::Result<()> {
            std::fs::create_dir_all(&self.dir)?;
            let enc = protect(data)?;
            let path = self.path(p);
            let tmp = path.with_extension("dat.tmp");
            {
                use std::io::Write;
                let mut f = std::fs::File::create(&tmp)?;
                f.write_all(&enc)?;
                f.sync_all()?;
            }
            std::fs::rename(&tmp, &path).inspect_err(|_| {
                let _ = std::fs::remove_file(&tmp);
            })
        }
        fn delete(&self, p: Platform) -> std::io::Result<()> {
            match std::fs::remove_file(self.path(p)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_roundtrip_and_flags() {
        let s = MemoryStore::default();
        let c = StoredCred { v: 1, refresh_token: "RT".into(), refresh_expires_at: "x".into(), account_id: "a".into(), display_name: "d".into() };
        save_cred(&s, &c).unwrap();
        assert_eq!(load_cred(&s).unwrap().unwrap().refresh_token, "RT");
        s.corrupt.store(true, Ordering::SeqCst);
        assert_eq!(load_cred(&s).unwrap_err(), AuthError::Corrupt);
    }
    #[test]
    fn wrong_version_is_not_connected() {
        let s = MemoryStore::default();
        s.save(Platform::Epic, br#"{"v":2,"refresh_token":"r","refresh_expires_at":"x","account_id":"a","display_name":"d"}"#).unwrap();
        assert_eq!(load_cred(&s).unwrap_err(), AuthError::NotConnected);
        assert!(s.raw(Platform::Epic).is_some());
        s.save(Platform::Epic, b"{not json").unwrap();
        assert_eq!(load_cred(&s).unwrap_err(), AuthError::Corrupt);
    }
    #[test]
    fn arc_store_delegates_and_fail_save() {
        let s = Arc::new(MemoryStore::default());
        s.fail_save.store(true, Ordering::SeqCst);
        assert!(SecretStore::save(&s, Platform::Epic, b"x").is_err());
        s.fail_save.store(false, Ordering::SeqCst);
        SecretStore::save(&s, Platform::Epic, b"x").unwrap();
        assert_eq!(s.saves(), 1);
        assert_eq!(SecretStore::load(&s, Platform::Epic).unwrap(), Some(b"x".to_vec()));
    }
    #[cfg(windows)]
    #[test]
    fn dpapi_roundtrip_and_corrupt() {
        let dir = std::env::temp_dir().join(format!("hangar-dpapi-{}", std::process::id()));
        let s = DpapiStore::new(dir.clone());
        s.save(Platform::Epic, b"FIRST-BYTES").unwrap();
        s.save(Platform::Epic, b"RT-PLAIN").unwrap();
        let raw = std::fs::read(dir.join("epic.dat")).unwrap();
        assert!(!raw.windows(8).any(|w| w == b"RT-PLAIN"));
        assert_eq!(s.load(Platform::Epic).unwrap().unwrap(), b"RT-PLAIN");
        std::fs::write(dir.join("epic.dat"), b"garbage").unwrap();
        assert_eq!(s.load(Platform::Epic).unwrap_err(), AuthError::Corrupt);
        s.delete(Platform::Epic).unwrap();
        assert_eq!(s.load(Platform::Epic).unwrap(), None);
        s.delete(Platform::Epic).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
    #[cfg(windows)]
    #[test]
    fn io_error_is_not_corrupt() {
        let dir = std::env::temp_dir().join(format!("hangar-dpapi-io-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("epic.dat")).unwrap();
        let s = DpapiStore::new(dir.clone());
        let e = s.load(Platform::Epic).unwrap_err();
        assert_ne!(e, AuthError::Corrupt);
        let _ = std::fs::remove_dir_all(dir);
    }
}
