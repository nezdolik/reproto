mod file_objects;
mod git_objects;
mod http_objects;

use errors::*;
use git;
pub use self::file_objects::FileObjects;
pub use self::git_objects::GitObjects;
pub use self::http_objects::HttpObjects;
use sha256::Checksum;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tokio_core::reactor::Core;
use url::Url;

/// Configuration file for objects backends.
pub struct ObjectsConfig {
    /// Root path when checking out local repositories.
    pub repos: Option<PathBuf>,
}

pub trait Objects {
    /// Put the given object into the database.
    /// This will cause the object denoted by the given checksum to be uploaded to the objects
    /// store.
    fn put_object(&mut self, checksum: &Checksum, source: &mut Read) -> Result<()>;

    /// Get a path to the object with the given checksum.
    /// This might cause the object to be downloaded if it's not already present in the local
    /// filesystem.
    fn get_object(&mut self, checksum: &Checksum) -> Result<Option<PathBuf>>;

    /// Update local caches related to the object store.
    fn update(&self) -> Result<()> {
        Ok(())
    }
}

pub fn objects_from_file<P: AsRef<Path>>(path: P) -> Result<FileObjects> {
    let path = path.as_ref();

    if !path.is_dir() {
        return Err(format!("no such directory: {}", path.display()).into());
    }

    Ok(FileObjects::new(path))
}

pub fn objects_from_git<'a, I>(config: ObjectsConfig,
                               scheme: I,
                               url: &'a Url)
                               -> Result<Box<Objects>>
    where I: IntoIterator<Item = &'a str>
{
    let mut scheme = scheme.into_iter();

    let sub_scheme = scheme.next()
        .ok_or_else(|| format!("invalid scheme ({}), expected git+scheme", url.scheme()))?;

    let repos = config.repos.ok_or_else(|| "repos: not specified")?;

    let git_repo = git::setup_git_repo(&repos, sub_scheme, url)?;

    let file_objects = FileObjects::new(git_repo.path());

    let git_repo = Rc::new(git_repo);
    let objects = GitObjects::new(url.clone(), git_repo, file_objects);

    Ok(Box::new(objects))
}

pub fn objects_from_http(_config: ObjectsConfig, url: &Url) -> Result<Box<Objects>> {
    let core = Core::new()?;
    let http_objects = HttpObjects::new(url.clone(), core);
    Ok(Box::new(http_objects))
}

pub fn objects_from_url(config: ObjectsConfig, url: &Url) -> Result<Box<Objects>> {
    let mut scheme = url.scheme().split("+");
    let first = scheme.next().ok_or_else(|| format!("invalid scheme: {}", url))?;

    match first {
        "file" => {
            let path = Path::new(url.path());
            objects_from_file(path).map(|objects| Box::new(objects) as Box<Objects>)
        }
        "git" => objects_from_git(config, scheme, url),
        "http" => objects_from_http(config, url),
        scheme => Err(format!("unsupported scheme ({}): {}", scheme, url).into()),
    }
}
