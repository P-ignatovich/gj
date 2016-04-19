// Copyright (c) 2013-2016 Sandstorm Development Group, Inc. and contributors
// Licensed under the MIT License:
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

use std::os::unix::io::RawFd;
use handle_table::{HandleTable, Handle};
use sys::FdObserver;
use nix::sys::epoll;

pub struct Reactor {
    pub ep: RawFd,
    pub observers: HandleTable<FdObserver>,
    events: Vec<epoll::EpollEvent>
}

impl Reactor {
    pub fn new() -> Result<Reactor, ::std::io::Error> {
        Ok(Reactor {
            ep: try!(epoll::epoll_create()),
            observers: HandleTable::new(),
            events: Vec::new(),
        })
    }

    pub fn run_once(&mut self) -> Result<(), ::std::io::Error> {
        unimplemented!()
    }

    pub fn new_observer(&mut self, fd: RawFd) -> Result<Handle, ::std::io::Error> {
        unimplemented!()
    }
}