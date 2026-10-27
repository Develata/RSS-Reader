//! Eval-channel failure handling; interaction policy remains in the controller.
use dioxus::prelude::document::{Eval, EvalError};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::controller::Fact;

#[derive(Serialize)]
struct Rejected {
    rejected: bool,
}

pub(super) async fn receive_fact(eval: &mut Eval) -> Result<Fact, EvalError> {
    receive_message(eval).await
}

pub(super) async fn receive_message<T: DeserializeOwned>(eval: &mut Eval) -> Result<T, EvalError> {
    loop {
        match eval.recv::<T>().await {
            Ok(fact) => return Ok(fact),
            Err(EvalError::Serialization(error)) => {
                // Eval already consumed this JSON message. Reject just this
                // request, without changing directory state or closing the
                // channel. Its private adapter has exactly one request in flight;
                // a sequence cannot be trusted when deserialization has failed.
                tracing::warn!(%error, "Rejected invalid directory message");
                eval.send(Rejected { rejected: true })?;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        collections::VecDeque,
        rc::Rc,
        task::{Context, Poll},
    };

    use dioxus::{core::internal::generational_box::Owner, prelude::document::Evaluator};
    use serde_json::{Value, json};

    use super::*;

    struct Channel {
        incoming: VecDeque<Value>,
        replies: Rc<RefCell<Vec<Value>>>,
        reject_send: bool,
    }

    impl Evaluator for Channel {
        fn send(&self, value: Value) -> Result<(), EvalError> {
            if self.reject_send {
                return Err(EvalError::Communication("closed test channel".into()));
            }
            self.replies.borrow_mut().push(value);
            Ok(())
        }

        fn poll_recv(&mut self, _: &mut Context<'_>) -> Poll<Result<Value, EvalError>> {
            Poll::Ready(self.incoming.pop_front().ok_or(EvalError::Finished))
        }

        fn poll_join(&mut self, _: &mut Context<'_>) -> Poll<Result<Value, EvalError>> {
            Poll::Ready(Err(EvalError::Finished))
        }
    }

    #[tokio::test]
    async fn malformed_messages_are_rejected_and_the_next_fact_reaches_the_controller() {
        let replies = Rc::new(RefCell::new(Vec::new()));
        let owner = Owner::default();
        let mut eval = Eval::new(owner.insert(Box::new(Channel {
            incoming: VecDeque::from([
                json!({"epoch":1,"sequence":1,"kind":"future_kind"}),
                json!({"epoch":1,"sequence":"damaged","kind":"observe"}),
                json!(null),
                json!([1, 2]),
                json!({}),
                json!({"epoch":1,"sequence":2,"kind":"visibility","visible":true}),
            ]),
            replies: replies.clone(),
            reject_send: false,
        }) as Box<dyn Evaluator>));
        let mut state = super::super::controller::DirectoryState::default();
        state.reset(None, None);
        let before = state.clone();
        let fact = receive_fact(&mut eval).await.unwrap();
        assert!(state == before);
        assert_eq!(*replies.borrow(), vec![json!({"rejected":true}); 5]);
        let reply = state.receive(&fact);
        assert!(reply.publish && state.is_visible());
    }

    #[tokio::test]
    async fn a_closed_receiver_is_terminal_without_retry_or_reply() {
        let replies = Rc::new(RefCell::new(Vec::new()));
        let owner = Owner::default();
        let mut eval = Eval::new(owner.insert(Box::new(Channel {
            incoming: VecDeque::new(),
            replies: replies.clone(),
            reject_send: false,
        }) as Box<dyn Evaluator>));
        assert!(matches!(receive_fact(&mut eval).await, Err(EvalError::Finished)));
        assert!(replies.borrow().is_empty());
    }

    #[tokio::test]
    async fn a_failed_rejection_reply_is_terminal() {
        let owner = Owner::default();
        let mut eval = Eval::new(owner.insert(Box::new(Channel {
            incoming: VecDeque::from([json!(null)]),
            replies: Rc::default(),
            reject_send: true,
        }) as Box<dyn Evaluator>));
        assert!(matches!(receive_fact(&mut eval).await, Err(EvalError::Communication(_))));
    }
}
