mod event;
mod receiver;
mod dispatcher;

#[cfg(test)]
mod tests {
    use super::{event::{EMessage}, dispatcher::EventDispatcher};

    fn callback(event: &EMessage) {
        println!("Message Received: ({}, {})", event.msg, event.count);
    }

    #[test]
    fn test_dispatch_events() {
        let mut dispatcher = EventDispatcher::new();

        let e1 = EMessage {
            count: 54,
            msg: String::from("message counting!"),
        };

        let e1_1 = EMessage {
            count: 233,
            msg: String::from("Invalid message!"),
        };

        dispatcher.connect_func::<EMessage>(callback);

        dispatcher.trigger(&e1);
        dispatcher.trigger(&e1_1);
    }
}