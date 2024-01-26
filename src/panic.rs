use std::panic;

use backtrace::Backtrace;

pub fn set_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        let payload = panic_info
            .payload()
            .downcast_ref::<&str>()
            .map(|v| (*v).to_string())
            .unwrap_or_default();

        let location = panic_info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_default();

        log::error!("panicked at '{}', {}", payload, location);
        log::error!("{:?}", Backtrace::new());
    }));
}
