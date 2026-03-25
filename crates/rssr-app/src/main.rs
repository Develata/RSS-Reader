mod app;
mod bootstrap;
mod components;
mod hooks;
mod pages;
mod router;
mod theme;

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    dioxus::launch(app::App);
}
