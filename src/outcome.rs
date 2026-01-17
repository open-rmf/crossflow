/*
 * Copyright (C) 2026 Open Source Robotics Foundation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
*/

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::sync::oneshot::{
    self,
    error::{RecvError, TryRecvError},
};

use crate::{Cancellation, CancellationCause};

/// Contains the final outcome of a [`Series`].
///
/// This mostly just wraps a oneshot receiver to have nicer ergonomics. If the
/// oneshot sender disconnects, its error message gets flattened into a regular
/// cancellation.
///
/// [`Series`]: crate::Series
pub struct Outcome<T> {
    inner: oneshot::Receiver<Result<T, Cancellation>>,
}

impl<T> Future for Outcome<T> {
    type Output = Result<T, Cancellation>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Future::poll(Pin::new(&mut self.get_mut().inner), cx).map(flatten_recv)
    }
}

impl<T> Outcome<T> {
    /// Try receiving the outcome if it's available. This can be used in a
    /// blocking context but does not block execution itself.
    ///
    /// If the outcome is not available yet, this will return None.
    ///
    /// If the outcome was previously delivered or if the sender was dropped,
    /// this will give a [`CancellationCause::Undeliverable`].
    pub fn try_recv(&mut self) -> Option<Result<T, Cancellation>> {
        flatten_try_recv(self.inner.try_recv())
    }

    /// Check if the outcome has already been delivered. If this is true then
    /// you will no longer be able to poll for the outcome.
    pub fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }

    /// Check if the outcome is available to be received.
    ///
    /// If the outcome was previously received this will return false. Use
    /// [`Self::is_terminated`] to tell pending outcomes apart from
    /// already-delivered outcomes.
    ///
    /// If you want to know if an outcome is still pending, use [`Self::is_pending`].
    pub fn is_available(&self) -> bool {
        !self.inner.is_empty()
    }

    /// Check if the outcome is still being determined.
    pub fn is_pending(&self) -> bool {
        self.inner.is_empty() && !self.is_terminated()
    }

    /// Make a new Outcome. This is usually created by [`Series`], but the API
    /// is public in case users have a reason to create one manually.
    pub fn new(receiver: oneshot::Receiver<Result<T, Cancellation>>) -> Self {
        Self { inner: receiver }
    }
}

fn flatten_recv<T>(
    response: Result<Result<T, Cancellation>, RecvError>,
) -> Result<T, Cancellation> {
    match response {
        Ok(r) => r,
        Err(err) => Err(err.into()),
    }
}

fn flatten_try_recv<T>(
    response: Result<Result<T, Cancellation>, TryRecvError>,
) -> Option<Result<T, Cancellation>> {
    match response {
        Ok(r) => Some(r),
        Err(err) => match err {
            TryRecvError::Empty => None,
            TryRecvError::Closed => Some(Err(CancellationCause::Undeliverable.into())),
        },
    }
}
