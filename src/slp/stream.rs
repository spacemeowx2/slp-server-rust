use crate::util::FilterSameExt;
use futures::prelude::*;
use tokio::sync::broadcast;
use tokio::time::Duration;
use tokio_stream::wrappers::IntervalStream;

pub fn spawn_stream<T, F, Fut, Item>(obj: &T, func: F) -> broadcast::Sender<Item>
where
    T: Clone + Send + 'static,
    F: Fn(T) -> Fut + Send + 'static,
    Fut: Future<Output = Item> + Send + 'static,
    Item: Clone + PartialEq + Send + 'static,
{
    let (tx, _) = broadcast::channel(1);
    let obj = obj.clone();
    let sender = tx.clone();

    tokio::spawn(
        IntervalStream::new(tokio::time::interval(Duration::from_secs(1)))
            .then(move |_| {
                let obj = obj.clone();
                func(obj)
            })
            .filter_same()
            .for_each(move |item| {
                // ignore the only error: no active receivers
                let _ = tx.send(item);
                future::ready(())
            }),
    );

    sender
}
