use std::{
    marker::PhantomData,
    ops::{ControlFlow, Deref},
    sync::Arc,
};

pub struct ChainPiece<Event, H, Next>
where
    H: Handler<Event>,
    Next: Handler<Event>,
{
    next: Next,
    handler: H,

    _event: PhantomData<Event>,
}

impl<Event, H, Next> ChainPiece<Event, H, Next>
where
    H: Handler<Event>,
    Next: Handler<Event>,
{
    pub fn new(handler: H) -> ChainPiece<Event, H, ()> {
        ChainPiece {
            next: (),
            handler,
            _event: Default::default(),
        }
    }
}

impl<Event, H, Next> Handler<Event> for ChainPiece<Event, H, Next>
where
    H: Handler<Event>,
    Next: Handler<Event>,
{
    fn handle_event(&self, event: Event) -> ControlFlow<(), Event> {
        use ControlFlow::*;

        match self.handler.handle_event(event) {
            Continue(message) => self.next.handle_event(message),
            Break(v) => Break(v),
        }
    }
}

pub trait Handler<E> {
    fn handle_event(&self, event: E) -> ControlFlow<(), E>;
}

impl<E> Handler<E> for () {
    #[inline]
    fn handle_event(&self, event: E) -> ControlFlow<(), E> {
        ControlFlow::Continue(event)
    }
}

impl<Event, T: Handler<Event>> Handler<Event> for Arc<T> {
    fn handle_event(&self, event: Event) -> ControlFlow<(), Event> {
        self.deref().handle_event(event)
    }
}

pub trait HandlerExt<Event>: Sized + Handler<Event> {
    fn chain_to<H: Handler<Event>>(self, new_handler: H) -> ChainPiece<Event, H, Self>;
}

impl<Event, T: Handler<Event>> HandlerExt<Event> for T {
    fn chain_to<H: Handler<Event>>(self, new_handler: H) -> ChainPiece<Event, H, Self> {
        ChainPiece {
            next: self,
            handler: new_handler,
            _event: Default::default(),
        }
    }
}

// pub trait EventHandler<Next> {
//     fn handle()
// }
