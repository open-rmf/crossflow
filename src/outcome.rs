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

use crate::{Cancellation, CancellationCause, CaptureOutcome};

/// Contains the final outcome of a [`Series`].
///
/// This mostly just wraps a oneshot receiver to have nicer ergonomics. If the
/// oneshot sender disconnects, its error message gets flattened into a regular
/// cancellation.
///
/// [`Series`]: crate::Series
pub struct Outcome<T> {
    /// A receiver to receive the actual result of the outcome
    value: oneshot::Receiver<Result<T, Cancellation>>,

    /// A receiver attached to a sender who is simply monitoring for whether
    /// the outcome gets dropped.
    finished: Option<oneshot::Sender<()>>,
}

impl<T> Future for Outcome<T> {
    type Output = Result<T, Cancellation>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let self_mut = self.get_mut();
        let r = Future::poll(Pin::new(&mut self_mut.value), cx).map(flatten_recv);

        match r {
            Poll::Pending => Poll::Pending,
            Poll::Ready(value) => {
                // If we are receiving the value then notify the finished monitor
                // that we successfully received the outcome.
                if let Some(finished) = self_mut.finished.take() {
                    let _ = finished.send(());
                }

                Poll::Ready(value)
            }
        }
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
        match self.value.try_recv() {
            Ok(r) => {
                if let Some(finished) = self.finished.take() {
                    let _ = finished.send(());
                }

                Some(r)
            }
            Err(err) => match err {
                TryRecvError::Empty => None,
                TryRecvError::Closed => {
                    if let Some(finished) = self.finished.take() {
                        let _ = finished.send(());
                    }

                    Some(Err(CancellationCause::Undeliverable.into()))
                }
            },
        }
    }

    /// Check if the outcome has already been delivered. If this is true then
    /// you will no longer be able to poll for the outcome.
    pub fn is_terminated(&self) -> bool {
        self.value.is_terminated()
    }

    /// Check if the outcome is available to be received.
    ///
    /// If the outcome was previously received this will return false. Use
    /// [`Self::is_terminated`] to tell pending outcomes apart from
    /// already-delivered outcomes.
    ///
    /// If you want to know if an outcome is still pending, use [`Self::is_pending`].
    pub fn is_available(&self) -> bool {
        !self.value.is_empty()
    }

    /// Check if the outcome is still being determined.
    pub fn is_pending(&self) -> bool {
        self.value.is_empty() && !self.is_terminated()
    }

    pub(crate) fn new() -> (Self, CaptureOutcome<T>) {
        let (value_sender, value_receiver) = oneshot::channel();
        let (finished_sender, finished_receiver) = oneshot::channel();

        let outcome = Self {
            value: value_receiver,
            finished: Some(finished_sender),
        };

        let capture = CaptureOutcome::new(value_sender, finished_receiver);

        (outcome, capture)
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
