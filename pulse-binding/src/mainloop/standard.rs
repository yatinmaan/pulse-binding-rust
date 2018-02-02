//! Standard/minimal main loop implementation based on poll().

// This file is part of the PulseAudio Rust language binding.
//
// Copyright (c) 2017 Lyndon Brown
//
// This library is free software; you can redistribute it and/or modify it under the terms of the
// GNU Lesser General Public License as published by the Free Software Foundation; either version
// 2.1 of the License, or (at your option) any later version.
//
// This library is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without
// even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License along with this library;
// if not, see <http://www.gnu.org/licenses/>.

//! # Overview
//!
//! This 'standard' (minimal) main loop implementation is based on the poll() system call. It
//! supports the functions defined in the main loop abstraction ([`::mainloop::api`]) and very
//! little else.
//!
//! This implementation is thread safe as long as you access the main loop object from a single
//! thread only.
//!
//! # Usage
//!
//! A [`Mainloop`] is created using [`Mainloop::new`]. To get access to the main loop abstraction,
//! [`Mainloop::get_api`] is used.
//!
//! Destruction of the [`Mainloop`] object is done automatically when the object falls out of scope.
//! (Rust's `Drop` trait has been implemented and takes care of it).
//!
//! # Iteration
//!
//! The main loop is designed around the concept of iterations. Each iteration consists of three
//! steps that repeat during the application's entire lifetime:
//!
//! * Prepare - Build a list of file descriptors that need to be monitored and calculate the next
//!   timeout.
//! * Poll - Execute the actual poll() system call.
//! * Dispatch - Dispatch any events that have fired.
//!
//! When using the main loop, the application can either execute each iteration, one at a time,
//! using [`Mainloop::iterate`], or let the library iterate automatically using [`Mainloop::run`].
//!
//! # Threads
//!
//! The main loop functions are designed to be thread safe, but the objects are not. What this means
//! is that multiple main loops can be used, but only one object per thread.
//!
//! # Example
//!
//! An example program using the standard mainloop:
//!
//! ```rust
//! extern crate libpulse_binding as pulse;
//!
//! use std::os::raw::c_void;
//! use std::sync::atomic;
//! use std::mem::transmute;
//! use pulse::mainloop::standard::Mainloop;
//! use pulse::stream::Stream;
//! use pulse::mainloop::standard::InterateResult;
//!
//! fn main() {
//!     let spec = pulse::sample::Spec {
//!         format: pulse::sample::SAMPLE_S16NE,
//!         channels: 2,
//!         rate: 44100,
//!     };
//!     assert!(spec.is_valid());
//!
//!     let mut proplist = pulse::proplist::Proplist::new().unwrap();
//!     proplist.sets(pulse::proplist::properties::APPLICATION_NAME, "FooApp")
//!         .unwrap();
//!
//!     let mainloop = Mainloop::new().unwrap();
//!
//!     let mut context = pulse::context::Context::new_with_proplist(
//!         mainloop.get_api(),
//!         "FooAppContext",
//!         &mut proplist
//!         ).unwrap();
//!
//!     context.connect(None, pulse::context::flags::NOFLAGS, None).unwrap();
//!
//!     // Wait for context to be ready
//!     loop {
//!         match mainloop.iterate(false) {
//!             InterateResult::Quit(_) |
//!             InterateResult::Err(_) => {
//!                 eprintln!("iterate state was not success, quitting...");
//!                 return;
//!             },
//!             InterateResult::Success(_) => {},
//!         }
//!         match context.get_state() {
//!             pulse::context::State::Ready => { break; },
//!             pulse::context::State::Failed |
//!             pulse::context::State::Terminated => {
//!                 eprintln!("context state failed/terminated, quitting...");
//!                 return;
//!             },
//!             _ => {},
//!         }
//!     }
//!
//!     let stream = Stream::new(&mut context, "Music", &spec, None).unwrap();
//!
//!     stream.connect_playback(None, None, pulse::stream::flags::START_CORKED,
//!         None, None).unwrap();
//!
//!     // Wait for stream to be ready
//!     loop {
//!         match mainloop.iterate(false) {
//!             InterateResult::Quit(_) |
//!             InterateResult::Err(_) => {
//!                 eprintln!("iterate state was not success, quitting...");
//!                 return;
//!             },
//!             InterateResult::Success(_) => {},
//!         }
//!         match stream.get_state() {
//!             pulse::stream::State::Ready => { break; },
//!             pulse::stream::State::Failed |
//!             pulse::stream::State::Terminated => {
//!                 eprintln!("stream state failed/terminated, quitting...");
//!                 return;
//!             },
//!             _ => {},
//!         }
//!     }
//!
//!     // Our main loop
//! #   let mut count = 0; // For automatic unit tests, we'll spin a few times
//!     let drained = atomic::AtomicBool::new(false);
//!     loop {
//!         match mainloop.iterate(false) {
//!             InterateResult::Quit(_) |
//!             InterateResult::Err(_) => {
//!                 eprintln!("iterate state was not success, quitting...");
//!                 return;
//!             },
//!             InterateResult::Success(_) => {},
//!         }
//!
//!         // Write some data with stream.write()
//!
//!         if stream.is_corked().unwrap() {
//!             stream.uncork(None);
//!         }
//!
//!         // Wait for our data to be played
//!         stream.drain(Some((drain_cb, unsafe { transmute(&drained) })));
//!         while !drained.compare_and_swap(true, false, atomic::Ordering::Relaxed) {
//!             match mainloop.iterate(false) {
//!                 InterateResult::Quit(_) |
//!                 InterateResult::Err(_) => {
//!                     eprintln!("iterate state was not success, quitting...");
//!                     return;
//!                 },
//!                 InterateResult::Success(_) => {},
//!             }
//!         }
//!
//!         // Remember to break out of the loop once done writing all data (or whatever).
//! #       // Stop test getting stuck in infinite loop!
//! #       count += 1;
//! #       if count == 3 {
//! #           break;
//! #       }
//!     }
//!
//!     // Clean shutdown
//!     mainloop.quit(0); // uncertain whether this is necessary
//!     stream.disconnect().unwrap();
//! }
//! 
//! extern "C"
//! fn drain_cb(_: *mut pulse::stream::StreamInternal, _: i32, d: *mut c_void) {
//!     assert!(!d.is_null());
//!     let signal: &atomic::AtomicBool = unsafe { transmute(d) };
//!     signal.store(true, atomic::Ordering::Relaxed);
//! }
//! ```
//!
//! [`::mainloop::api`]: ../api/index.html
//! [`Mainloop`]: struct.Mainloop.html
//! [`Mainloop::new`]: struct.Mainloop.html#method.new
//! [`Mainloop::get_api`]: struct.Mainloop.html#method.get_api
//! [`Mainloop::iterate`]: struct.Mainloop.html#method.iterate
//! [`Mainloop::run`]: struct.Mainloop.html#method.run

use std;
use capi;
use std::os::raw::{c_ulong, c_void};
use std::rc::Rc;
use std::ptr::null_mut;
use libc::pollfd;

pub use capi::pa_mainloop as MainloopInternal;

impl super::api::MainloopInternalType for MainloopInternal {}

/// Generic prototype of a poll() like function
pub type PollFn = extern "C" fn(ufds: *mut pollfd, nfds: c_ulong, timeout: i32,
    userdata: *mut c_void) -> i32;

/// Return type for [`Mainloop::iterate`](struct.Mainloop.html#method.iterate).
pub enum InterateResult {
    /// Success, with number of sources dispatched
    Success(u32),
    /// Quit was called, with quit's retval
    Quit(i32),
    /// An error occurred, with error value
    Err(i32),
}

impl InterateResult {
    /// Returns `true` if the result is a `Success` value.
    #[inline]
    pub fn is_success(&self) -> bool {
        match *self {
            InterateResult::Success(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if the result is a `Quit` value.
    #[inline]
    pub fn is_quit(&self) -> bool {
        match *self {
            InterateResult::Quit(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if the result is an `Error` value.
    #[inline]
    pub fn is_error(&self) -> bool {
        match *self {
            InterateResult::Err(_) => true,
            _ => false,
        }
    }
}

/// This acts as a safe interface to the internal PA Mainloop.
///
/// The mainloop object pointers are further enclosed here in a ref counted wrapper, allowing this
/// outer wrapper to have clean methods for creating event objects, which can cleanly pass a copy of
/// the inner ref counted mainloop object to them. Giving this to events serves two purposes,
/// firstly because they need the API pointer, secondly, it ensures that event objects do not
/// outlive the mainloop object.
pub struct Mainloop {
    /// The ref-counted inner data
    pub _inner: Rc<super::api::MainloopInner<MainloopInternal>>,
}

impl super::api::Mainloop for Mainloop {
    type MI = super::api::MainloopInner<MainloopInternal>;

    fn inner(&self) -> Rc<super::api::MainloopInner<MainloopInternal>> {
        self._inner.clone()
    }
}

impl super::api::MainloopInner<MainloopInternal> {
    fn drop_actual(&mut self) {
        unsafe { capi::pa_mainloop_free(self.ptr) };
        self.ptr = null_mut::<MainloopInternal>();
        self.api = null_mut::<::mainloop::api::MainloopApi>();
    }
}

impl Mainloop {
    /// Allocate a new main loop object
    pub fn new() -> Option<Self> {
        let ptr = unsafe { capi::pa_mainloop_new() };
        if ptr.is_null() {
            return None;
        }
        let api_ptr = unsafe { capi::pa_mainloop_get_api(ptr) };
        assert!(!api_ptr.is_null());
        Some(
            Self {
                _inner: Rc::new(
                    super::api::MainloopInner::<MainloopInternal> {
                        ptr: ptr,
                        api: unsafe { std::mem::transmute(api_ptr) },
                        dropfn: super::api::MainloopInner::<MainloopInternal>::drop_actual,
                    }
                ),
            }
        )
    }

    /// Prepare for a single iteration of the main loop.
    ///
    /// Returns `Err` on error or exit request.
    ///
    /// `timeout` specifies a maximum timeout for the subsequent poll, or `None` for blocking
    /// behaviour. Only positive values should be provided, negative values will have the same
    /// effect as `None`.
    pub fn prepare(&self, timeout: Option<i32>) -> Result<(), i32> {
        let t: i32 = match timeout {
            Some(t) => t ,
            None => -1,
        };
        match unsafe { capi::pa_mainloop_prepare((*self._inner).ptr, t) } {
            0 => Ok(()),
            e => Err(e),
        }
    }

    /// Execute the previously prepared poll.
    pub fn poll(&self) -> Result<u32, i32> {
        match unsafe { capi::pa_mainloop_poll((*self._inner).ptr) } {
            e if e >= 0 => Ok(e as u32),
            e => Err(e),
        }
    }

    /// Dispatch timeout, io and deferred events from the previously executed poll. On success
    /// returns the number of source dispatched.
    pub fn dispatch(&self) -> Result<u32, i32> {
        match unsafe { capi::pa_mainloop_dispatch((*self._inner).ptr) } {
            e if e >= 0 => Ok(e as u32),
            e => Err(e),
        }
    }

    /// Return the return value as specified with the main loop's [`quit`](#method.quit) routine.
    pub fn get_retval(&self) -> i32 {
        unsafe { capi::pa_mainloop_get_retval((*self._inner).ptr) }
    }

    /// Run a single iteration of the main loop.
    ///
    /// This is a convenience function for [`prepare`](#method.prepare), [`poll`](#method.poll)
    /// and [`dispatch`](#method.dispatch).
    ///
    /// If `block` is `true`, block for events if none are queued.
    ///
    /// Returns an [`InterateResult`](enum.InterateResult.html) variant:
    ///
    /// * On success, returns `InterateResult::Success` containing the number of sources dispatched
    ///   in this iteration.
    /// * If exit was requested, returns `InterateResult::Quit` containing quit's retval.
    /// * On error, returns `InterateResult::Err` containing error value.
    pub fn iterate(&self, block: bool) -> InterateResult {
        let mut retval: i32 = 0;
        match unsafe { capi::pa_mainloop_iterate((*self._inner).ptr, block as i32, &mut retval) } {
            r if r >= 0 => InterateResult::Success(r as u32),
            -2 => InterateResult::Quit(retval),
            r => InterateResult::Err(r),
        }
    }

    /// Run unlimited iterations of the main loop object until the main loop's
    /// [`quit`](#method.quit) routine is called.
    ///
    /// On success, returns `Some` containing quit's retval. On error returns `None`.
    pub fn run(&self) -> Option<i32> {
        let mut retval: i32 = 0;
        match unsafe { capi::pa_mainloop_run((*self._inner).ptr, &mut retval) } {
            r if r >= 0 => Some(r),
            _ => None,
        }
    }

    /// Return the abstract main loop abstraction layer vtable for this main loop.
    ///
    /// No need to free the API as it is owned by the loop and is destroyed when the loop is freed.
    ///
    /// Talking to PA directly with C requires fetching this pointer explicitly via this function.
    /// This is actually unecessary through this binding. The pointer is retrieved automatically
    /// upon Mainloop creation, stored internally, and automatically obtained from it by functions
    /// that need it.
    pub fn get_api(&self) -> &mut ::mainloop::api::MainloopApi {
        let ptr = (*self._inner).api;
        assert_eq!(false, ptr.is_null());
        unsafe { &mut *ptr }
    }

    /// Shutdown the main loop with the specified return value
    pub fn quit(&self, retval: i32) {
        unsafe { capi::pa_mainloop_quit((*self._inner).ptr, retval); }
    }

    /// Interrupt a running poll (for threaded systems)
    pub fn wakeup(&self) {
        unsafe { capi::pa_mainloop_wakeup((*self._inner).ptr); }
    }

    /// Change the poll() implementation
    pub fn set_poll_func(&self, poll_cb: (PollFn, *mut c_void)) {
        unsafe { capi::pa_mainloop_set_poll_func((*self._inner).ptr, Some(poll_cb.0), poll_cb.1); }
    }
}