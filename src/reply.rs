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

use tracing::error;

use tokio::sync::oneshot;

/// Contains the reply of a closure sent using the async [`Channel`].
///
/// This mostly just wraps a oneshot receiver to have nicer ergonomics. A normal
/// oneshot recevier has the possibility of yielding a [`RecvError`] if the
/// sender gets dropped, but there is no way the sender will be dropped under
/// ordinary operation of crossflow. Therefore the [`Future`] of `Reply` only
/// outputs the [`Ok`] variant of the receiver.
///
/// If the sender for a [`Reply`] is somehow dropped, that [`Reply`] will never
/// yield a value, but it will print an error. Again it should not be possible
/// for this to happen if crossflow is implemented correctly. Please open an
/// issue ticket if you ever see that a [`Reply`] has stalled out.
///
/// [`Channel`]: crate::Channel
pub struct Reply<T> {
    inner: oneshot::Receiver<T>,
}

impl<T> Future for Reply<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Future::poll(Pin::new(&mut self.get_mut().inner), cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(value)) => Poll::Ready(value),
            Poll::Ready(Err(_)) => {
                error!(
                    "A reply from the async channel has stalled out because \
                    its sender dropped. Please report this as a bug.");
                // Just pass back pending. This future can never yield a value
                // at this point.
                Poll::Pending
            }
        }

    }
}

impl<T> Reply<T> {
    /// Try receiving the outcome if it's available. This can be used in a
    /// blocking context but does not block execution itself.
    ///
    /// If the outcome is not available yet, this will return None.
    ///
    /// If the outcome was previously delivered or if the sender was dropped,
    /// this will give a [`CancellationCause::Undeliverable`].
    pub fn try_recv(&mut self) -> Option<T> {
        self.inner.try_recv().ok()
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

    /// Make a new Reply. This is only supposed to be used by a [`Channel`],
    /// because a [`Channel`] can guarantee that the Future will be fulfilled.
    ///
    /// [`Channel`]: crate::Channel
    pub(crate) fn new(receiver: oneshot::Receiver<T>) -> Self {
        Self { inner: receiver }
    }
}
