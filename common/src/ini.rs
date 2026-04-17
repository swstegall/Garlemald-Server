//! Thin wrapper over the `rust-ini` crate that preserves the typed
//! getter/setter shape of `STA_INIFile.cs`. The Project Meteor call sites
//! (e.g. `ConfigConstants.cs`) treat every value as `GetValue(section, key, default)`,
//! so we mirror that with overload-by-trait.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ini::Ini;

pub struct IniFile {
    path: PathBuf,
    inner: Mutex<Ini>,
    auto_flush: bool,
}

impl IniFile {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::open_with(path, false, false)
    }

    pub fn open_with(
        path: impl AsRef<Path>,
        lazy: bool,
        auto_flush: bool,
    ) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let inner = if lazy || !path.exists() {
            Ini::new()
        } else {
            Ini::load_from_file(&path).map_err(io::Error::other)?
        };
        Ok(Self { path, inner: Mutex::new(inner), auto_flush })
    }

    pub fn get_value<T: IniValue>(&self, section: &str, key: &str, default: T) -> T {
        let guard = self.inner.lock().expect("ini mutex poisoned");
        let raw = guard.get_from(Some(section), key);
        match raw {
            Some(s) => T::parse(s).unwrap_or(default),
            None => default,
        }
    }

    pub fn set_value<T: IniValue>(&self, section: &str, key: &str, value: T) -> io::Result<()> {
        {
            let mut guard = self.inner.lock().expect("ini mutex poisoned");
            guard
                .with_section(Some(section))
                .set(key, value.render());
        }
        if self.auto_flush {
            self.flush()?;
        }
        Ok(())
    }

    pub fn flush(&self) -> io::Result<()> {
        let guard = self.inner.lock().expect("ini mutex poisoned");
        let tmp = self.path.with_extension("$n$");
        guard.write_to_file(&tmp).map_err(io::Error::other)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

pub trait IniValue: Sized {
    fn parse(raw: &str) -> Option<Self>;
    fn render(&self) -> String;
}

impl IniValue for String {
    fn parse(raw: &str) -> Option<Self> {
        Some(raw.to_owned())
    }
    fn render(&self) -> String {
        self.clone()
    }
}

impl IniValue for bool {
    fn parse(raw: &str) -> Option<Self> {
        raw.trim().parse::<i64>().ok().map(|n| n != 0)
    }
    fn render(&self) -> String {
        if *self { "1".into() } else { "0".into() }
    }
}

macro_rules! impl_ini_numeric {
    ($($t:ty),* $(,)?) => {
        $(
            impl IniValue for $t {
                fn parse(raw: &str) -> Option<Self> { raw.trim().parse().ok() }
                fn render(&self) -> String { self.to_string() }
            }
        )*
    };
}

impl_ini_numeric!(i32, u32, i64, u64, f32, f64);
