use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use kpi::io::*;
use x86::bits64::paging::VAddr;

use crate::arch::process::{UserPtr, UserValue};
use crate::fs::{Mnode, Modes};

/// Each memory-node can be of two types: directory or a file.
#[derive(Debug, Eq, PartialEq)]
pub enum NodeType {
    Directory,
    File,
}

/// Memnode representation, similar to Inode for a memory-fs.
#[derive(Debug, Eq, PartialEq)]
pub struct MemNode {
    mnode_num: Mnode,
    name: String,
    node_type: NodeType,
    file: Option<File>,
}

impl MemNode {
    /// Initialize a memory-node for a directory or a file.
    pub fn new(mnode_num: Mnode, pathname: &str, modes: Modes, node_type: NodeType) -> MemNode {
        let file = match node_type {
            NodeType::Directory => None,
            NodeType::File => Some(File::new(modes)),
        };

        MemNode {
            mnode_num,
            name: pathname.to_string(),
            node_type,
            file,
        }
    }

    /// Write to an in-memory file.
    pub fn write(&mut self, buffer: u64, len: u64) -> u64 {
        // Return if the user doesn't have write permissions for the file.
        if self.node_type != NodeType::File
            || !is_allowed!(self.file.as_ref().unwrap().modes, S_IWUSR)
        {
            return 0;
        }
        let buffer: *const u8 = buffer as *const u8;
        let len: usize = len as usize;

        let user_slice = unsafe { &mut core::slice::from_raw_parts(buffer, len) };
        let userval = UserValue::new(user_slice);
        self.file
            .as_mut()
            .unwrap()
            .data
            .append(&mut userval.to_vec());

        len as u64
    }

    /// Read from an in-memory file.
    pub fn read(&mut self, buffer: u64, len: u64) -> u64 {
        let modes = self.file.as_ref().unwrap().modes;
        // Return if the user doesn't have read permissions for the file.
        if !is_allowed!(modes, S_IRUSR) {
            return 0;
        }
        let mut user_ptr = VAddr::from(buffer);
        let slice_ptr = UserPtr::new(&mut user_ptr);
        let len: usize = len as usize;

        let user_slice = unsafe { core::slice::from_raw_parts_mut(slice_ptr.as_mut_ptr(), len) };
        user_slice.copy_from_slice(self.file.as_ref().unwrap().data.as_slice().split_at(len).0);

        len as u64
    }
}

/// An in-memory file, which is just a vector and stores permissions.
#[derive(Debug, Eq, PartialEq)]
pub struct File {
    data: Vec<u8>,
    modes: Modes,
    // TODO: Add more file related attributes
}

impl File {
    pub fn new(modes: Modes) -> File {
        File {
            data: Vec::new(),
            modes,
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::alloc::borrow::ToOwned;
    use crate::alloc::vec::Vec;
    use core::sync::atomic::Ordering;
    use kpi::io::*;

    #[test]
    /// Initialize a file and check the permissions.
    fn test_init_file() {
        let file = File::new(ALL_PERM);
        assert_eq!(file.modes, ALL_PERM);
        assert_eq!(file.data.len(), 0);
    }

    #[test]
    /// Create mnode directory and verify the values.
    fn test_mnode_directory() {
        let filename = "dir";
        let mut memnode = MemNode::new(1, filename, ALL_PERM, NodeType::Directory);
        assert_eq!(memnode.file, None);
        assert_eq!(memnode.mnode_num, 1);
        assert_eq!(memnode.name, filename.to_string());
        assert_eq!(memnode.node_type, NodeType::Directory);
    }

    #[test]
    /// Create mnode file and verify the values.
    fn test_mnode_file() {
        let filename = "file.txt";
        let mut memnode = MemNode::new(1, filename, ALL_PERM, NodeType::File);
        assert_eq!(memnode.file, Some(File::new(ALL_PERM)));
        assert_eq!(memnode.mnode_num, 1);
        assert_eq!(memnode.name, filename.to_string());
        assert_eq!(memnode.node_type, NodeType::File);
    }

    #[test]
    fn test_mnode_write_directory() {
        let filename = "dir";
        let mut memnode = MemNode::new(1, filename, ALL_PERM, NodeType::Directory);
        assert_eq!(memnode.file, None);
        assert_eq!(memnode.mnode_num, 1);
        assert_eq!(memnode.name, filename.to_string());
        assert_eq!(memnode.node_type, NodeType::Directory);
        let buffer: &[u8; 10] = &[0xb; 10];
        assert_eq!(memnode.write(buffer.as_ptr() as u64, 10), 0);
    }

    #[test]
    /// Write to mnode file and verify the values.
    fn test_mnode_file_write() {
        let filename = "file.txt";
        let mut memnode = MemNode::new(1, filename, ALL_PERM, NodeType::File);
        assert_eq!(memnode.file, Some(File::new(ALL_PERM)));
        assert_eq!(memnode.mnode_num, 1);
        assert_eq!(memnode.name, filename.to_string());
        assert_eq!(memnode.node_type, NodeType::File);
        let buffer: &[u8; 10] = &[0xb; 10];
        assert_eq!(memnode.write(buffer.as_ptr() as u64, 10), 10);
    }

    #[test]
    /// Write to mnode file which doesn't have write permissions.
    fn test_mnode_file_write_permission_error() {
        let filename = "file.txt";
        let mut memnode = MemNode::new(1, filename, S_IRUSR, NodeType::File);
        assert_eq!(memnode.file, Some(File::new(S_IRUSR)));
        assert_eq!(memnode.mnode_num, 1);
        assert_eq!(memnode.name, filename.to_string());
        assert_eq!(memnode.node_type, NodeType::File);
        let buffer: &[u8; 10] = &[0xb; 10];
        assert_eq!(memnode.write(buffer.as_ptr() as u64, 10), 0);
    }

    #[test]
    /// Read from mnode file.
    fn test_mnode_file_read() {
        let filename = "file.txt";
        let mut memnode = MemNode::new(1, filename, S_IRWXU, NodeType::File);
        assert_eq!(memnode.file, Some(File::new(S_IRWXU)));
        assert_eq!(memnode.mnode_num, 1);
        assert_eq!(memnode.name, filename.to_string());
        assert_eq!(memnode.node_type, NodeType::File);
        let buffer: &[u8; 10] = &[0xb; 10];
        assert_eq!(memnode.write(buffer.as_ptr() as u64, 10), 10);
        let mut buffer: &mut [u8; 10] = &mut [0; 10];
        assert_eq!(memnode.read(buffer.as_ptr() as u64, 10), 10);
        assert_eq!(buffer[0], 0xb);
        assert_eq!(buffer[9], 0xb);
    }

    #[test]
    /// Read from mnode file which doesn't have read permissions.
    fn test_mnode_file_read_permission_error() {
        let filename = "file.txt";
        let mut memnode = MemNode::new(1, filename, S_IWUSR, NodeType::File);
        assert_eq!(memnode.file, Some(File::new(S_IWUSR)));
        assert_eq!(memnode.mnode_num, 1);
        assert_eq!(memnode.name, filename.to_string());
        assert_eq!(memnode.node_type, NodeType::File);
        let buffer: &[u8; 10] = &[0xb; 10];
        assert_eq!(memnode.read(buffer.as_ptr() as u64, 10), 0);
    }
}