use std::borrow::Cow;
use std::path::Path;

use git2::ErrorClass;
use git2::ErrorCode;

use crate::Error;
use crate::Pointer;

pub trait RepoLfsExt {
  fn get_lfs_blob_content<'r>(&self, blob: &'r git2::Blob<'_>) -> Result<Cow<'r, [u8]>, Error>;
}

pub trait BlobLfsExt {
  fn is_lfs_pointer(&self) -> bool;
}

impl RepoLfsExt for git2::Repository {
  fn get_lfs_blob_content<'r>(&self, blob: &'r git2::Blob<'_>) -> Result<Cow<'r, [u8]>, Error> {
    let Some(pointer) = Pointer::from_str_short(blob.content()) else {
      return Ok(Cow::Borrowed(blob.content()));
    };

    let path = self.path().join("lfs/objects").join(pointer.path());

    if !path.exists() {
      let err = git2::Error::new(
        ErrorCode::NotFound,
        ErrorClass::Odb,
        format!(
          "object '{}' contains lfs pointer but the target object '{}' wasn't found (tried {})",
          blob.id(),
          pointer.hex(),
          Path::new("lfs/objects").join(pointer.path()).display()
        ),
      );

      return Err(err.into());
    }

    let content = std::fs::read(path)?;
    Ok(Cow::Owned(content))
  }
}

impl BlobLfsExt for git2::Blob<'_> {
  fn is_lfs_pointer(&self) -> bool {
    Pointer::is_pointer(self.content())
  }
}
