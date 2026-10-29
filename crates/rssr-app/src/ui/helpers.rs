use dioxus::prelude::*;

use super::UiIntent;

pub(crate) fn apply_projected_ui_intents<T>(
    intents: Vec<UiIntent>,
    project: fn(UiIntent) -> Option<T>,
    mut apply: impl FnMut(T),
) {
    for intent in intents.into_iter().filter_map(project) {
        apply(intent);
    }
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
