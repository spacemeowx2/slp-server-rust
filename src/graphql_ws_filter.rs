use juniper::{ScalarValue};
use juniper_warp::subscriptions::graphql_subscriptions;
use juniper_subscriptions::Coordinator;
use std::pin::Pin;
use std::sync::Arc;
use warp::{filters::BoxedFilter, Filter};
use futures::{Future, FutureExt as _};

pub fn make_graphql_ws_filter<Query, Mutation, Subscription, Context, S>(
    schema: juniper::RootNode<'static, Query, Mutation, Subscription, S>,
    context_extractor: BoxedFilter<(Context,)>,
) -> BoxedFilter<(impl warp::Reply, )>
where
    S: ScalarValue + Send + Sync + 'static,
    Context: Send + Sync + Clone + 'static,
    Query: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
    Query::TypeInfo: Send + Sync,
    Mutation: juniper::GraphQLTypeAsync<S, Context = Context> + Send + Sync + 'static,
    Mutation::TypeInfo: Send + Sync,
    Subscription: juniper::GraphQLSubscriptionType<S, Context = Context> + Send + Sync + 'static,
    Subscription::TypeInfo: Send + Sync,
{
    let coordinator = Arc::new(juniper_subscriptions::Coordinator::new(schema));

    let filter = (warp::ws()
        .and(context_extractor)
        .and(warp::any().map(move || Arc::clone(&coordinator)))
        .map(
            |ws: warp::ws::Ws,
             ctx: Context,
             coordinator: Arc<Coordinator<'static, _, _, _, _, _>>| {
                ws.on_upgrade(|websocket| -> Pin<Box<dyn Future<Output = ()> + Send>> {
                    graphql_subscriptions(websocket, coordinator, ctx).boxed()
                })
            },
        ))
        .map(|reply| {
            warp::reply::with_header(reply, "Sec-WebSocket-Protocol", "graphql-ws")
        });

    return filter.boxed()
}
