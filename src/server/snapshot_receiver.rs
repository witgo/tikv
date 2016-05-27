// Copyright 2016 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fmt::{self, Formatter, Display};
use std::{io, fs};
use std::boxed::{Box, FnBox};
use std::io::Read;

use super::{Result, Error};
use util::worker::{Runnable, Worker};
use bytes::{ByteBuf, MutByteBuf};
use byteorder::{ByteOrder, LittleEndian};
use protobuf::Message;
use raftstore::store::worker::snap::snapshot_file_path;

use kvproto::raftpb::Snapshot;
use kvproto::msgpb::SnapshotFile;
use kvproto::raft_serverpb::RaftMessage;

pub type Callback = Box<FnBox(Result<u64>) + Send>;

// TODO make it zero copy
pub struct Task {
    buf: ByteBuf,
    cb: Callback,
    last: bool,
}

impl Task {
    pub fn new(buf: &[u8], cb: Callback, last: bool) -> Task {
        Task {
            buf: ByteBuf::from_slice(buf),
            cb: cb,
            last: last,
        }
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Snapshot File Receiver Task")
    }
}

pub struct Runner {
    file_name: String,
    pub file: fs::File,
    msg: RaftMessage,
    file_info: SnapshotFile,
}

impl Runner {
    pub fn new(path: &str, file_info: SnapshotFile, msg: RaftMessage) -> Runner {
        let file_name = snapshot_file_path(path, 0, &file_info);
        Runner {
            file_name: file_name.to_owned(),
            file: fs::File::create(file_name).unwrap(),
            msg: msg,
            file_info: file_info,
        }
    }
}

impl Runnable<Task> for Runner {
    fn run(&mut self, task: Task) {
        let mut buf = task.buf;
        let resp = io::copy(&mut buf, &mut self.file);
        if task.last {
            // self.file.flush();

            // // TODO change here when region size goes to 1G
            // let snapshot = load_snapshot(&self.file_name, &self.file_info);
            // ch.send(Msg::Snapshot{snapshot});
        }
        task.cb.call_box((resp.map_err(Error::Io),))
    }
}

fn load_snapshot(file_name: &str) -> Result<Snapshot> {
    let mut file = fs::File::open(file_name).unwrap();
    let mut buf = [0; 4];
    try!(file.read(&mut buf));
    let len = LittleEndian::read_u32(&buf);

    let mut vec: Vec<u8> = Vec::with_capacity(10);
    try!(file.read(vec.as_mut_slice()));

    let mut msg = Snapshot::new();
    try!(msg.merge_from_bytes(vec.as_slice()));
    Ok(msg)
}

pub struct SnapshotReceiver {
    pub worker: Worker<Task>,
    pub buf: MutByteBuf,
    pub more: bool,
}
