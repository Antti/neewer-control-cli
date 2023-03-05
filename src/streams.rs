use crate::btle_plat;
use btleplug::api::{CentralEvent, ValueNotification};
use std::{pin, task};
use tokio_stream::Stream;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum StreamKey {
    BtleEvents,
    BtleNotifications(btle_plat::PeripheralId),
}

#[derive(Debug)]
pub enum EventVariants {
    Event(CentralEvent),
    Notification(ValueNotification),
}

pub enum EventStreams {
    BtleEvents(pin::Pin<Box<dyn Stream<Item = CentralEvent> + Send>>),
    BtleNotifications(pin::Pin<Box<dyn Stream<Item = ValueNotification> + Send>>),
}

impl Unpin for EventStreams {}

impl Stream for EventStreams {
    type Item = EventVariants;

    fn poll_next(
        mut self: pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let mut this = self.as_mut();

        match &mut *this {
            Self::BtleEvents(stream) => stream
                .as_mut()
                .poll_next(cx)
                .map(|x| x.map(Self::Item::Event)),
            Self::BtleNotifications(stream) => stream
                .as_mut()
                .poll_next(cx)
                .map(|x| x.map(Self::Item::Notification)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::BtleEvents(stream) => stream.size_hint(),
            Self::BtleNotifications(stream) => stream.size_hint(),
        }
    }
}
