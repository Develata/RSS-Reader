use super::{UiCommand, UiIntent, execute_ui_command};

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
