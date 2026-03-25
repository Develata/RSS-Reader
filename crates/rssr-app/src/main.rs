mod app;
mod bootstrap;
mod components;
mod pages;
mod router;

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    dioxus::launch(app::App);
}
