use std::collections::HashSet;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use git2::Filter;
use git2::FilterMode;
use git2::FilterRepository;

use crate::Error;

use tracing::*;

use crate::Pointer;

#[derive(Clone, Debug)]
pub struct LfsBuilder {
  relative_objects_dir: PathBuf,
  exts: Option<HashSet<String>>,
  max_file_size: Option<u64>,
}

pub struct Lfs<'a> {
  config: &'a LfsBuilder,
  repo: FilterRepository,
}

impl<'a> Lfs<'a> {
  pub fn new(repo: FilterRepository, config: &'a LfsBuilder) -> Self {
    Self { config, repo }
  }

  pub fn check(self, path: &Path) -> Result<bool, Error> {
    if let Some(exts) = &self.config.exts
      && let Some(ext) = path.extension().and_then(|value| value.to_str())
    {
      return Ok(exts.contains(ext));
    }

    if let Some(max_file_size) = self.config.max_file_size {
      let size = path.metadata()?.len();
      return Ok(size <= max_file_size);
    }

    Ok(false)
  }

  pub fn clean(self, input: &[u8], out: &mut impl Write) -> Result<(), Error> {
    let pointer = Pointer::from_blob_bytes(input)?;
    self.store_object_if_not_exists(&pointer, input)?;
    pointer.write_pointer(out)?;

    Ok(())
  }

  pub fn smudge(self, input: &[u8], out: &mut impl Write) -> Result<(), Error> {
    let pointer = Pointer::from_str(std::str::from_utf8(input)?)?;
    self.load_object(&pointer, out)?;
    Ok(())
  }

  fn store_object_if_not_exists(self, pointer: &Pointer, bytes: &[u8]) -> Result<(), Error> {
    let path = self.object_dir().join(pointer.path());

    if path.exists() {
      return Ok(());
    }

    pointer.write_blob_bytes(&self.object_dir(), bytes)?;
    Ok(())
  }

  fn load_object(self, pointer: &Pointer, out: &mut impl Write) -> Result<(), Error> {
    let path = self.object_dir().join(pointer.path());

    if !path.exists() {
      warn!(path = %path.display(), "object not found, skipping");
      return Ok(());
    }

    let file = std::fs::File::open(&path)?;
    let mut reader = BufReader::new(file);
    std::io::copy(&mut reader, out)?;
    Ok(())
  }

  fn object_dir(&self) -> PathBuf {
    self.repo.path().join(&self.config.relative_objects_dir)
  }
}

impl Default for LfsBuilder {
  fn default() -> Self {
    Self {
      relative_objects_dir: Path::new("lfs").join("objects"),
      exts: Default::default(),
      max_file_size: Default::default(),
    }
  }
}

impl LfsBuilder {
  pub fn with_objects_dir(mut self, objects_dir: PathBuf) -> Self {
    self.relative_objects_dir = objects_dir;
    self
  }

  pub fn with_file_extensions(mut self, exts: &[&str]) -> Self {
    self.exts = Some(exts.iter().map(|ext| ext.to_string()).collect());
    self
  }

  pub fn with_max_file_size(mut self, max_file_size: u64) -> Self {
    self.max_file_size = Some(max_file_size);
    self
  }

  pub fn install(self) -> Result<(), Error> {
    let mut filter = Filter::<()>::new()?;

    let config = Arc::new(self);

    let on_check_config = Arc::clone(&config);
    let on_apply_config = Arc::clone(&config);

    filter
      .on_init(|_| Ok(()))
      .on_check(move |_, _, src, _| {
        let lfs = Lfs::new(src.repo(), &on_check_config);

        let Some(path) = src.path() else {
          warn!("filter did't provide a path, skipping");
          return Ok(false);
        };

        let check = lfs.check(path).expect("FIX ME");

        Ok(check)
      })
      .on_apply(move |_, _, mut to, from, src| {
        let lfs = Lfs::new(src.repo(), &on_apply_config);

        match src.mode() {
          FilterMode::Clean => {
            lfs.clean(from.as_bytes(), &mut to.as_allocated_vec()).expect("FIX ME");
            Ok(())
          }
          FilterMode::Smudge => {
            if !Pointer::is_pointer(from.as_bytes()) {
              let buf = to.as_allocated_vec();
              buf.extend_from_slice(from.as_bytes());
              return Ok(());
            }

            lfs.smudge(from.as_bytes(), &mut to.as_allocated_vec()).expect("FIX ME");
            Ok(())
          }
        }
      });

    filter.register("lfs", 1)?;
    Ok(())
  }
}
