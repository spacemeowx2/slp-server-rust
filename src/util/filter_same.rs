use futures::future;
use futures::{stream::BoxStream, StreamExt};

impl<T: ?Sized> FilterSameExt for T
where
    T: StreamExt,
    T::Item: Clone + PartialEq,
{
}

pub trait FilterSameExt: StreamExt {
    fn filter_same<'a>(self) -> BoxStream<'a, Self::Item>
    where
        Self::Item: 'a + Clone + PartialEq + Send,
        Self: 'a + Sized + Send,
    {
        let state: Option<Self::Item> = None;
        self.scan(state, |state, x| {
            future::ready(Some(if state.as_ref() == Some(&x) {
                None
            } else {
                *state = Some(x.clone());
                Some(x)
            }))
        })
        .filter_map(|x| future::ready(x))
        .boxed()
    }
}
