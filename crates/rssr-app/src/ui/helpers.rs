use dioxus::prelude::*;

use super::{UiCommand, UiIntent, execute_ui_command};

pub(crate) async fn visit_ui_command(command: UiCommand, mut visit: impl FnMut(UiIntent)) {
    for intent in execute_ui_command(command).await {
        visit(intent);
    }
}

pub(crate) async fn collect_projected_ui_command<T>(
    command: UiCommand,
    project: fn(UiIntent) -> Option<T>,
) -> Vec<T> {
    execute_ui_command(command).await.into_iter().filter_map(project).collect()
}

pub(crate) fn apply_projected_ui_intents<T>(
    intents: Vec<UiIntent>,
    project: fn(UiIntent) -> Option<T>,
    mut apply: impl FnMut(T),
) {
    for intent in intents.into_iter().filter_map(project) {
        apply(intent);
    }
}

pub(crate) async fn apply_projected_ui_command<T>(
    command: UiCommand,
    project: fn(UiIntent) -> Option<T>,
    apply: impl FnMut(T),
) {
    let intents = execute_ui_command(command).await;
    apply_projected_ui_intents(intents, project, apply);
}

pub(crate) fn spawn_ui_command(command: UiCommand, apply: impl FnOnce(Vec<UiIntent>) + 'static) {
    spawn(async move {
        apply(execute_ui_command(command).await);
    });
}

pub(crate) fn spawn_projected_ui_command<T: 'static>(
    command: UiCommand,
    project: fn(UiIntent) -> Option<T>,
    apply: impl FnMut(T) + 'static,
) {
    spawn(async move {
        apply_projected_ui_command(command, project, apply).await;
    });
}

pub(crate) fn use_reactive_task<T>(dependency: T, mut task: impl FnMut(T) + Copy + 'static)
where
    T: Clone + PartialEq + 'static,
{
    use_resource(use_reactive!(|(dependency)| async move {
        task(dependency.clone());
    }));
}

pub(crate) fn use_reactive_side_effect<T>(dependency: T, mut effect: impl FnMut(T) + Copy + 'static)
where
    T: Clone + PartialEq + 'static,
{
    use_effect(use_reactive!(|(dependency)| {
        effect(dependency.clone());
    }));
}
