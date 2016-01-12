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

//! Unix domain sockets.

use std::result::Result;
use handle_table::Handle;
use io::{FdObserver, register_new_handle, Error};
use {EventLoop, Promise, WaitScope};
use private::with_current_event_loop;

pub type Stream = ::io::stream::Stream<::mio::unix::UnixStream>;

pub struct Listener {
    listener: ::mio::unix::UnixListener,
    handle: Handle,
    no_send: ::std::marker::PhantomData<*mut ()>, // impl !Send for Listener
}

impl Drop for Listener {
    fn drop(&mut self) {
        with_current_event_loop(move |event_loop| {
            event_loop.event_port.borrow_mut().handler.observers.remove(self.handle);
            let _ = event_loop.event_port.borrow_mut().reactor.deregister(&self.listener);
        })
    }
}

impl Listener {
    pub fn bind<P: AsRef<::std::path::Path>>(addr: P) -> Result<Listener, ::std::io::Error> {
        let listener = try!(::mio::unix::UnixListener::bind(&addr));
        let handle = FdObserver::new();

        with_current_event_loop(move |event_loop| {
            try!(event_loop.event_port
                           .borrow_mut()
                           .reactor
                           .register(&listener,
                                     ::mio::Token(handle.val),
                                     ::mio::EventSet::readable(),
                                     ::mio::PollOpt::edge()));
            Ok(Listener {
                listener: listener,
                handle: handle,
                no_send: ::std::marker::PhantomData,
            })
        })
    }

    fn accept_internal(self) -> Promise<(Listener, Stream), Error<Listener>> {
        let accept_result = match self.listener.accept() {
            Err(e) => return Promise::err((Error::new(self, e))),
            Ok(v) => v,
        };
        match accept_result {
            Some(stream) => {
                let handle = match register_new_handle(&stream) {
                    Err(e) => return Promise::err(Error::new(self, e)),
                    Ok(v) => v,
                };
                Promise::ok((self, Stream::new(stream, handle)))
            }
            None => {
                with_current_event_loop(move |event_loop| {
                    let promise = event_loop.event_port.borrow_mut().handler.observers[self.handle]
                                      .when_becomes_readable();
                    promise.then_else(move |r| {
                        match r {
                            Ok(()) => self.accept_internal(),
                            Err(e) => Promise::err(Error::new(self, e)),
                        }
                    })
                })
            }
        }
    }

    pub fn accept(self) -> Promise<(Listener, Stream), Error<Listener>> {
        Promise::ok(()).then(move |()| self.accept_internal())
    }
}

impl Stream {
    pub fn connect<P: AsRef<::std::path::Path>>(addr: P) -> Promise<Stream, ::std::io::Error> {
        let connect_result = ::mio::unix::UnixStream::connect(&addr);
        Promise::ok(()).then(move |()| {
            let stream = pry!(connect_result);

            // TODO: if we're not already connected, maybe only register writable interest,
            // and then reregister with read/write interested once we successfully connect.

            let handle = pry!(register_new_handle(&stream));

            with_current_event_loop(move |event_loop| {
                let promise = event_loop.event_port.borrow_mut().handler.observers[handle]
                                  .when_becomes_writable();
                promise.map(move |()| {
                    // try!(stream.take_socket_error());
                    Ok(Stream::new(stream, handle))
                })
            })
        })
    }

    pub fn new_pair() -> Result<(Stream, Stream), ::std::io::Error> {
        use nix::sys::socket::{socketpair, AddressFamily, SockType, SOCK_CLOEXEC, SOCK_NONBLOCK};
        use std::os::unix::io::FromRawFd;

        let (fd1, fd2) = try!(socketpair(AddressFamily::Unix,
                                         SockType::Stream,
                                         0,
                                         SOCK_NONBLOCK | SOCK_CLOEXEC)
                                  .map_err(|_| {
                                      ::std::io::Error::new(::std::io::ErrorKind::Other,
                                                            "failed to create socketpair")
                                  }));

        unsafe {
            Ok((try!(Stream::from_raw_fd(fd1)),
                try!(Stream::from_raw_fd(fd2))))
        }
    }

    pub fn try_clone(&self) -> Result<Stream, ::std::io::Error> {
        let stream = try!(self.stream().try_clone());
        let handle = try!(register_new_handle(&stream));
        Ok(Stream::new(stream, handle))
    }

    /// Consumes a raw file descriptor to creates a `Stream`. Ensures that O_NONBLOCK is set on the
    /// descriptor.
    pub unsafe fn from_raw_fd(fd: ::std::os::unix::io::RawFd) -> Result<Stream, ::std::io::Error> {
        try!(::nix::fcntl::fcntl(fd,
                                 ::nix::fcntl::FcntlArg::F_SETFL(::nix::fcntl::O_NONBLOCK)));
        let stream = ::std::os::unix::io::FromRawFd::from_raw_fd(fd);
        let handle = try!(register_new_handle(&stream));
        Ok(Stream::new(stream, handle))
    }
}

/// Creates a new thread and sets up a socket pair that can be used to communicate with it.
/// Passes one of the sockets to the thread's start function and returns the other socket.
/// The new thread will already have an active event loop when `start_func` is called.
pub fn spawn<F>(start_func: F)
                -> Result<(::std::thread::JoinHandle<()>, Stream), Box<::std::error::Error>>
    where F: FnOnce(Stream, &WaitScope) -> Result<(), Box<::std::error::Error>>,
          F: Send + 'static
{
    use nix::sys::socket::{socketpair, AddressFamily, SockType, SOCK_CLOEXEC, SOCK_NONBLOCK};
    use std::os::unix::io::FromRawFd;

    let (fd0, fd1) = try!(socketpair(AddressFamily::Unix,
                                     SockType::Stream,
                                     0,
                                     SOCK_NONBLOCK | SOCK_CLOEXEC));

    let socket_stream = try!(unsafe { Stream::from_raw_fd(fd0) });

    let join_handle = ::std::thread::spawn(move || {
        let _result = EventLoop::top_level(move |wait_scope| {
            let socket_stream = try!(unsafe { Stream::from_raw_fd(fd1) });
            start_func(socket_stream, &wait_scope)
        });
    });

    return Ok((join_handle, socket_stream));
}

pub type Reader = ::io::stream::Reader<::mio::unix::UnixStream>;

pub type Writer = ::io::stream::Writer<::mio::unix::UnixStream>;
