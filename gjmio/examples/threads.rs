// Copyright (c) 2013-2015 Sandstorm Development Group, Inc. and contributors
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


//! Tasks scheduled on a GJ event loop are not preemptive. For an event loop to make progress,
//! event callbacks must yield control by returning.
//!
//! GJ event loops are thread-local. To take advantage of multiprocessor hardware or to deal with
//! tasks that cannot easily yield, you can send tasks to separate threads where they will execute
//! on separate event loops. The example program illustrates how that might work, using
//! `std::thread::sleep_ms()` as a stand-in for a blocking computation.

extern crate gj;
extern crate gjmio;

use gj::Promise;
use gjmio::{AsyncRead, AsyncWrite, unix};
use std::time::Duration;

fn child_loop(delay: Duration,
              stream: unix::Stream,
              buf: Vec<u8>) -> Promise<(), gjmio::Error<(unix::Stream, Vec<u8>)>> {

    // This blocks the entire thread. This is okay because we are on a child thread
    // where nothing else needs to happen.
    ::std::thread::sleep(delay);

    stream.write(buf).then(move |(stream, buf)| {
        child_loop(delay, stream, buf)
    })
}

fn child(delay: Duration) -> Result<unix::Stream, Box<::std::error::Error>> {
    let (_, stream) = try!(unix::spawn(move |parent_stream, wait_scope| {
        let mut event_port = gjmio::EventPort::new().unwrap(); // XXX?
        try!(child_loop(delay, parent_stream, vec![0u8]).lift::<Box<::std::error::Error>>().wait(wait_scope, &mut event_port));
        Ok(())
    }));
    return Ok(stream);
}

fn listen_to_child(id: &'static str,
                   stream: unix::Stream,
                   buf: Vec<u8>) -> Promise<(), gjmio::Error<(unix::Stream, Vec<u8>)>> {
    stream.read(buf, 1).then(move |(stream, buf, _n)| {
        println!("heard back from {}", id);
        listen_to_child(id, stream, buf)
    })
}

fn parent_wait_loop() -> Promise<(), ::std::io::Error> {
    println!("parent wait loop...");

    // If we used ::std::thread::sleep() here, we would block the main event loop.
    gjmio::Timer.after_delay(Duration::from_millis(3000)).then(|()| {
        parent_wait_loop()
    })
}

pub fn main() {
    gj::EventLoop::top_level(|wait_scope| -> Result<(), Box<::std::error::Error>> {
        let mut event_port = gjmio::EventPort::new().unwrap();
        let children = vec![
            parent_wait_loop().lift::<Box<::std::error::Error>>(),
            listen_to_child("CHILD 1", try!(child(Duration::from_millis(700))), vec![0]).lift(),
            listen_to_child("CHILD 2", try!(child(Duration::from_millis(1900))), vec![0]).lift(),
            listen_to_child("CHILD 3", try!(child(Duration::from_millis(2600))), vec![0]).lift()];

        try!(Promise::all(children.into_iter()).wait(wait_scope, &mut event_port));

        Ok(())
    }).unwrap();
}