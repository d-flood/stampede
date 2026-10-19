use std::{fs::File, io, os::fd::AsRawFd, path::{Path, PathBuf}};

pub struct SocketPath {
    path: PathBuf,
    _directory: Option<File>,
}

impl AsRef<Path> for SocketPath {
    fn as_ref(&self) -> &Path { &self.path }
}

pub fn path(path: &Path) -> io::Result<SocketPath> {
    if path.as_os_str().len() < 100 {
        return Ok(SocketPath { path: path.to_owned(), _directory: None });
    }
    let directory = File::open(path.parent().ok_or_else(|| io::Error::other("socket has no directory"))?)?;
    let name = path.file_name().ok_or_else(|| io::Error::other("socket has no filename"))?;
    let short = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd())).join(name);
    Ok(SocketPath { path: short, _directory: Some(directory) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::{UnixListener, UnixStream};

    #[test]
    fn a_long_socket_path_connects_through_its_directory() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("a".repeat(120));
        std::fs::create_dir(&directory).unwrap();
        let socket = directory.join("test.sock");
        let address = path(&socket).unwrap();
        let listener = UnixListener::bind(&address).unwrap();
        let _connection = UnixStream::connect(&address).unwrap();
        listener.accept().unwrap();
        assert!(socket.exists());
    }
}
