//! The core module for file management.

mod file;
mod name;

use alloc::string::String;
use alloc::string::ToString;
use core::sync::atomic::{AtomicUsize, Ordering};
use cstr_core::CStr;
use hashbrown::HashMap;
use kpi::io::*;
use x86::bits64::paging::VAddr;

use crate::arch::process::UserPtr;
use crate::fs::file::{MemNode, NodeType};

/// The maximum number of open files for a process.
pub const MAX_FILES_PER_PROCESS: usize = 8;

/// Mnode number.
pub type Mnode = u64;
/// Flags for fs calls.
pub type Flags = u64;
/// Modes for fs calls
pub type Modes = u64;
/// File descriptor.
pub type FD = u64;
/// Userspace buffer pointer to read or write a file.
pub type Buffer = u64;
/// Number of bytes to read or write a file.
pub type Len = u64;
/// Userspace-pointer to filename.
pub type Filename = u64;

/// Abstract definition of a file descriptor.
pub trait FileDescriptor {
    fn init_fd() -> Fd;
    fn update_fd(&mut self, mnode: Mnode, flags: Flags);
    fn get_mnode(&self) -> Mnode;
    fn get_flags(&self) -> Flags;
}

/// A file descriptor representation.
#[derive(Debug, Default)]
pub struct Fd {
    mnode: Mnode,
    flags: Flags,
}

impl FileDescriptor for Fd {
    fn init_fd() -> Fd {
        Fd {
            // Intial values are just the place-holders and shouldn't be used.
            mnode: core::u64::MAX,
            flags: 0,
        }
    }

    fn update_fd(&mut self, mnode: Mnode, flags: Flags) {
        self.mnode = mnode;
        self.flags = flags;
    }

    fn get_mnode(&self) -> Mnode {
        self.mnode.clone()
    }

    fn get_flags(&self) -> Flags {
        self.flags.clone()
    }
}

/// The in-memory file-system representation.
#[derive(Debug)]
pub struct MemFS {
    mnodes: HashMap<Mnode, MemNode>,
    files: HashMap<String, Mnode>,
    root: (String, Mnode),
    nextmemnode: AtomicUsize,
}

impl MemFS {
    /// Initialize the file system from the root directory.
    pub fn init() -> MemFS {
        let rootdir = "/";
        let rootmnode = 1;

        let mut mnodes = HashMap::new();
        mnodes.insert(
            rootmnode,
            MemNode::new(rootmnode, rootdir, ALL_PERM, NodeType::Directory),
        );
        let mut files = HashMap::new();
        files.insert(rootdir.to_string(), 1);
        let root = (rootdir.to_string(), 1);

        MemFS {
            mnodes,
            files,
            root,
            nextmemnode: AtomicUsize::new(2),
        }
    }

    /// Get the next available memnode number.
    fn get_next_mno(&mut self) -> usize {
        self.nextmemnode.fetch_add(1, Ordering::Relaxed)
    }

    /// Create a file in the root directory.
    pub fn create(&mut self, pathname: Filename, modes: Modes) -> Option<u64> {
        let mut user_ptr = VAddr::from(pathname);
        let str_ptr = UserPtr::new(&mut user_ptr);

        // TODO: Assume that all files are in the root directory.
        // Later, parse the full path into directory and file.
        let filename;
        unsafe {
            match CStr::from_ptr(str_ptr.as_mut_ptr()).to_str() {
                Ok(path) => {
                    filename = path;
                }
                Err(_) => unreachable!("FileCreate: Unable to convert u64 to str"),
            }
        }

        // Check if the file with the same name already exists.
        match self.files.get(&filename.to_string()) {
            Some(_) => return None,
            None => {}
        }

        let mnode_num = self.get_next_mno() as u64;
        let memnode = MemNode::new(mnode_num, filename, modes, NodeType::File);
        self.files.insert(filename.to_string(), mnode_num);
        self.mnodes.insert(mnode_num, memnode);

        Some(mnode_num)
    }

    /// Write data to a file.
    pub fn write(&mut self, mnode_num: Mnode, buffer: Buffer, len: Len) -> u64 {
        match self.mnodes.get_mut(&mnode_num) {
            Some(mnode) => mnode.write(buffer, len),
            None => 0,
        }
    }

    /// Read data from a file.
    pub fn read(&mut self, mnode_num: Mnode, buffer: Buffer, len: Len) -> u64 {
        match self.mnodes.get_mut(&mnode_num) {
            Some(mnode) => mnode.read(buffer, len),
            None => 0,
        }
    }

    /// Check if a file exists in the file system or not.
    pub fn lookup(&self, pathname: u64) -> (bool, Option<Mnode>) {
        let mut user_ptr = VAddr::from(pathname);
        let str_ptr = UserPtr::new(&mut user_ptr);

        // TODO: Assume that all files are in the root directory.
        // Later, parse the full path into directory and file.
        let filename;
        unsafe {
            match CStr::from_ptr(str_ptr.as_mut_ptr()).to_str() {
                Ok(path) => {
                    filename = path;
                }
                Err(_) => unreachable!("FileCreate: Unable to convert u64 to str"),
            }
        }

        match self.files.get(&filename.to_string()) {
            Some(mnode) => (true, Some(*mnode)),
            None => (false, None),
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::alloc::borrow::ToOwned;
    use crate::alloc::vec::Vec;
    use core::sync::atomic::Ordering;
    use core::u64::MAX;
    use kpi::io::*;

    #[test]
    /// Initialize and update file descriptor mnode number and permission flags.
    fn test_file_descriptor() {
        let mut fd = Fd::init_fd();
        assert_eq!(fd.get_mnode(), MAX);
        assert_eq!(fd.get_flags(), 0);

        fd.update_fd(1, O_RDWR);
        assert_eq!(fd.get_mnode(), 1);
        assert_eq!(fd.get_flags(), O_RDWR);
    }

    #[test]
    /// Initialize memfs for root and verify the values.
    fn test_memfs_init() {
        let memfs = MemFS::init();
        let root = String::from("/");
        assert_eq!(memfs.root, (root.to_owned(), 1));
        assert_eq!(memfs.nextmemnode.load(Ordering::Relaxed), 2);
        assert_eq!(memfs.files.get(&root), Some(&1));
        assert_eq!(
            memfs.mnodes.get(&1),
            Some(&MemNode::new(1, "/", ALL_PERM, NodeType::Directory))
        );
    }

    #[test]
    /// Create a file on in-memory fs and verify all the values.
    fn test_file_create() {
        let mut memfs = MemFS::init();
        let filename = "file.txt\0";
        let mnode = memfs.create(filename.as_ptr() as u64, S_IRUSR).unwrap();
        assert_eq!(mnode, 2);
        assert_eq!(memfs.nextmemnode.load(Ordering::Relaxed), 3);
        assert_eq!(memfs.files.get(&String::from("file.txt")), Some(&2));
    }

    #[test]
    /// Create a file with non-read permission and try to read it.
    fn test_file_read_permission_error() {
        let buffer = &[0; 10];
        let mut memfs = MemFS::init();
        let filename = "file.txt\0";
        let mnode = memfs.create(filename.as_ptr() as u64, S_IWUSR).unwrap();
        assert_eq!(mnode, 2);
        assert_eq!(memfs.nextmemnode.load(Ordering::Relaxed), 3);
        assert_eq!(memfs.files.get(&String::from("file.txt")), Some(&2));
        // On error read returns 0.
        assert_eq!(memfs.read(2, buffer.as_ptr() as u64, 10), 0);
    }

    #[test]
    /// Create a file with non-write permission and try to write it.
    fn test_file_write_permission_error() {
        let mut buffer = &[0; 10];
        let mut memfs = MemFS::init();
        let filename = "file.txt\0";
        let mnode = memfs.create(filename.as_ptr() as u64, S_IRUSR).unwrap();
        assert_eq!(mnode, 2);
        assert_eq!(memfs.nextmemnode.load(Ordering::Relaxed), 3);
        assert_eq!(memfs.files.get(&String::from("file.txt")), Some(&2));
        // On error read returns 0.
        assert_eq!(memfs.write(2, buffer.as_ptr() as u64, 10), 0);
    }

    #[test]
    /// Create a file and write to it.
    fn test_file_write() {
        let mut buffer = &[0; 10];
        let mut memfs = MemFS::init();
        let filename = "file.txt\0";
        let mnode = memfs.create(filename.as_ptr() as u64, ALL_PERM).unwrap();
        assert_eq!(mnode, 2);
        assert_eq!(memfs.nextmemnode.load(Ordering::Relaxed), 3);
        assert_eq!(memfs.files.get(&String::from("file.txt")), Some(&2));
        assert_eq!(memfs.write(2, buffer.as_ptr() as u64, 10), 10);
    }

    #[test]
    /// Create a file, write to it and then later read. Verify the content.
    fn test_file_read() {
        let len = 10;
        let wbuffer: &[u8; 10] = &[0xb; 10];
        let mut rbuffer: &mut [u8; 10] = &mut [0; 10];

        let mut memfs = MemFS::init();
        let filename = "file.txt\0";
        let mnode = memfs.create(filename.as_ptr() as u64, ALL_PERM).unwrap();
        assert_eq!(mnode, 2);
        assert_eq!(memfs.nextmemnode.load(Ordering::Relaxed), 3);
        assert_eq!(memfs.files.get(&String::from("file.txt")), Some(&2));
        assert_eq!(
            memfs.write(2, wbuffer.as_ptr() as u64, len as u64),
            len as u64
        );
        assert_eq!(
            memfs.read(2, rbuffer.as_ptr() as u64, len as u64),
            len as u64
        );
        assert_eq!(rbuffer[0], 0xb);
        assert_eq!(rbuffer[9], 0xb);
    }

    #[test]
    /// Create a file and lookup for it.
    fn test_file_lookup() {
        let mut memfs = MemFS::init();
        let filename = "file.txt\0";
        let mnode = memfs.create(filename.as_ptr() as u64, ALL_PERM).unwrap();
        assert_eq!(mnode, 2);
        assert_eq!(memfs.nextmemnode.load(Ordering::Relaxed), 3);
        assert_eq!(memfs.files.get(&String::from("file.txt")), Some(&2));
        let (is_present, mnode) = memfs.lookup(filename.as_ptr() as u64);
        assert_eq!(is_present, true);
        assert_eq!(mnode, Some(2));
    }

    #[test]
    /// Lookup for a fake file.
    fn test_file_fake_lookup() {
        let mut memfs = MemFS::init();
        let filename = "file.txt\0";
        let mnode = memfs.create(filename.as_ptr() as u64, ALL_PERM).unwrap();
        assert_eq!(mnode, 2);
        assert_eq!(memfs.nextmemnode.load(Ordering::Relaxed), 3);
        assert_eq!(memfs.files.get(&String::from("file.txt")), Some(&2));
        let (is_present, mnode) = memfs.lookup("filename".as_ptr() as u64);
        assert_eq!(is_present, false);
        assert_eq!(mnode, None);
    }

    #[test]
    /// Try to create a file with same name.
    fn test_file_duplicate_create() {
        let mut memfs = MemFS::init();
        let filename = "file.txt\0";
        let mnode = memfs.create(filename.as_ptr() as u64, ALL_PERM).unwrap();
        assert_eq!(mnode, 2);
        assert_eq!(memfs.nextmemnode.load(Ordering::Relaxed), 3);
        assert_eq!(memfs.files.get(&String::from("file.txt")), Some(&2));
        assert_eq!(memfs.create(filename.as_ptr() as u64, ALL_PERM), None);
    }
}