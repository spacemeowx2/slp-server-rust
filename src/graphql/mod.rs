mod model;

pub use model::{QueryRoot, SubscriptionRoot};

pub struct Data {
  online: i32
}

impl Data {
  pub fn new() -> Self {
    Data {
      online: 0,
    }
  }
}
